//! Adjoint (reverse-mode) evaluation of a single-terminal tail.
//!
//! # Tail
//!
//! A *tail* of one request is a suffix of phases `[start, end)` whose checked
//! steps emit terminal contributions to at most one terminal `T` (detected
//! during the run from the steps themselves; no phase, owner or terminal is
//! named). Children always enter the same or a later phase, so the frontier
//! at the start of `start` holds every weight that will ever enter the tail:
//! the *entry weights* `w(K)`. Every identity `K` of the tail has a complete
//! reduction `u(K) = sum_c coef(K, c) u(c) + coef(K, T)` onto `T` (over all
//! its children, whatever the request), a function of `K` and the program
//! alone, and the forward propagation of the tail adds exactly
//! `sum_K w(K) u(K)` to `T`: propagation is linear and an erased (cancelled)
//! key carries weight zero, so it contributes zero on either side.
//!
//! # Guards
//!
//! The forward pass retains the guards of the keys it expands (popped with a
//! nonzero merged weight), in pop order. Those decisions are kept exactly:
//!
//! - a run that expands the tail makes them itself;
//! - a run that closes the tail from the table without expanding it either
//!   needs none (every guard of every tabulated reduction is retained
//!   already, so expanding could not add or move one: any arithmetic), or
//!   repeats them: recorded arithmetics decide every erasure by "the probe
//!   of the merged weight is zero" (two values modulo a 61-bit prime); the
//!   table holds the tail's steps, so the scheduler sums the same probe
//!   values over the same linear map (finite-field sums do not depend on
//!   their order) and obtains the same decisions. Keys whose decision could
//!   matter but cannot be certified (a reached key decided zero that carries
//!   a guard not yet retained, see below) make it expand the tail instead.
//!
//! # What changes
//!
//! - Recorded (modular) weights: the recorded program of the tail is
//!   replaced by one product per entry key, `T += w(K) * u(K)`, with `u`
//!   computed exactly by back-substitution over the expanded steps (completed
//!   by the steps of reached keys the forward pass erased) or read from the
//!   table. Prime replays, which cost `edges x weight length` in the tail,
//!   then cost one product per entry. The value no longer depends on any
//!   erasure decision; the replayed erasure checks of the tail certified the
//!   decisions, which now only matter for guards, so a run in which a reached
//!   key erased by the forward pass carries a guard not retained at the start
//!   of the tail keeps its forward program (and its checks).
//! - With a persistent table (attached through a step cache, see
//!   [`table`]): at the start of every phase the scheduler checks whether the
//!   table covers the whole frontier with a single terminal and the guards
//!   allow closing (above); then it closes the tail at once. Otherwise it
//!   expands the tail as above and adds the new entries. Reached keys found
//!   in the table take their values from it and are not stepped; the
//!   erasure check above walks their tabulated steps (stopping at keys the
//!   forward pass expanded; entries it cannot attribute to the run, such as
//!   ones another request adds concurrently, are walked, which is
//!   conservative). So for a tail the table does not close (another
//!   request's tail) the guard check decides as in a run without the table.
//!   A table that closes the tail only at a phase after its start keeps the
//!   forward program before that phase.
//!
//! Terms and nonzero conditions (values and order) are therefore those of
//! the forward run. Progress counters of a closed tail count the keys the
//! forward pass would expand (probe closing) or every entry key (guard-free
//! closing), and the terms of their steps; merge and cancellation counters
//! and phase snapshots after the closing differ from the forward run.
pub(crate) mod table;
#[cfg(test)]
mod tests;

use std::cmp::Reverse;
use std::collections::{BTreeSet, HashMap};
use std::hash::{BuildHasher, Hash};
use std::sync::Arc;
use std::time::Instant;

use hashbrown::HashTable;

use self::table::TailData;
pub use self::table::{
    ORDERED_TAIL_SCHEMA, ORDERED_TAIL_SEMANTICS, OrderedTailTable, OrderedTailTableStatistics,
};
use super::parallel::Workers;
use super::step_cache::codec::FxBuild;
use super::{
    Admitted, OrderedKernel, OrderedPosition, OrderedStep, OrderedWeights, Scheduled, invariant,
    merge_terminal_contribution, replace_monomials,
};
use crate::algebra::{Coefficient, CoefficientContext, ExactAlgebraLimits};
use crate::family::IntegralKey;
use crate::reduction::{ReductionError, ReductionRequest, ReductionStatistics};

const NONE: u32 = u32::MAX;
/// Smallest level of independent tail values evaluated on the workers.
const PARALLEL_LEVEL: usize = 8;

/// `RUSTRED_ORDERED_ADJOINT_TAIL`: `1`/`on`/`true` enables and
/// `0`/`off`/`false` disables the adjoint tail for every request, whatever
/// the scheduler options say; unset (or another value): the options decide.
pub(super) fn environment() -> Option<bool> {
    static SETTING: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *SETTING.get_or_init(
        || match std::env::var("RUSTRED_ORDERED_ADJOINT_TAIL").as_deref() {
            Ok("1" | "on" | "true") => Some(true),
            Ok("0" | "off" | "false") => Some(false),
            _ => None,
        },
    )
}

/// The environment enables the adjoint tail for every request.
#[cfg(test)]
pub(super) fn forced_by_environment() -> bool {
    environment() == Some(true)
}

/// Process-local interning (FxHash) of hashable values.
pub(crate) struct Interner<T> {
    table: HashTable<u32>,
    values: Vec<T>,
    hashes: Vec<u64>,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Self {
            table: HashTable::new(),
            values: Vec::new(),
            hashes: Vec::new(),
        }
    }
}

impl<T: Hash + Eq> Interner<T> {
    fn hash(value: &T) -> u64 {
        FxBuild::default().hash_one(value)
    }

    pub(crate) fn get(&self, value: &T) -> Option<u32> {
        let hash = Self::hash(value);
        self.table
            .find(hash, |&id| self.values[id as usize] == *value)
            .copied()
    }

    /// The id of `value` (cloned in when new) and whether it was new.
    pub(crate) fn intern_ref(&mut self, value: &T) -> (u32, bool)
    where
        T: Clone,
    {
        let hash = Self::hash(value);
        if let Some(&id) = self
            .table
            .find(hash, |&id| self.values[id as usize] == *value)
        {
            return (id, false);
        }
        let id = u32::try_from(self.values.len()).expect("fewer than 2^32 interned values");
        self.values.push(value.clone());
        self.hashes.push(hash);
        let hashes = &self.hashes;
        self.table
            .insert_unique(hash, id, |&index| hashes[index as usize]);
        (id, true)
    }

    pub(crate) fn value(&self, id: u32) -> &T {
        &self.values[id as usize]
    }

    pub(crate) fn values(&self) -> &[T] {
        &self.values
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
}

/// Guard sets of one run: bit sets over the run's guard ids, interned.
#[derive(Default)]
struct GuardSets {
    sets: Interner<Box<[u64]>>,
}

impl GuardSets {
    fn intern(&mut self, mut words: Vec<u64>) -> u32 {
        while words.last() == Some(&0) {
            words.pop();
        }
        self.sets.intern_ref(&words.into_boxed_slice()).0
    }

    fn of(ids: impl IntoIterator<Item = u32>) -> Vec<u64> {
        let mut words = Vec::new();
        for id in ids {
            let word = id as usize / 64;
            if words.len() <= word {
                words.resize(word + 1, 0);
            }
            words[word] |= 1u64 << (id % 64);
        }
        words
    }

    fn union_into(words: &mut Vec<u64>, other: &[u64]) {
        if words.len() < other.len() {
            words.resize(other.len(), 0);
        }
        for (word, &bits) in words.iter_mut().zip(other) {
            *word |= bits;
        }
    }

    fn members(words: &[u64]) -> impl Iterator<Item = u32> + '_ {
        words.iter().enumerate().flat_map(|(index, &word)| {
            (0..64)
                .filter(move |bit| word & (1u64 << bit) != 0)
                .map(move |bit| (index * 64 + bit) as u32)
        })
    }
}

/// One expanded identity of the tail: ranges into the log's edge, terminal
/// and guard lists.
struct LoggedStep {
    edges: (u32, u32),
    terminals: (u32, u32),
    guards: (u32, u32),
    /// Expanded by the forward pass (else: stepped to complete the tail).
    forward: bool,
}

/// The steps of the current candidate tail, in expansion order.
struct Log<K, P> {
    keys: Interner<K>,
    position: Vec<Option<OrderedPosition<P>>>,
    /// Index into `steps`, or `NONE`.
    step: Vec<u32>,
    steps: Vec<LoggedStep>,
    /// `(child, coefficient)`.
    edges: Vec<(u32, u32)>,
    /// `(terminal, coefficient)`.
    terminal_edges: Vec<(u32, u32)>,
    guard_refs: Vec<u32>,
    coefficients: Interner<Coefficient>,
    guards: Interner<Coefficient>,
    terminals: Interner<IntegralKey>,
}

impl<K, P> Default for Log<K, P> {
    fn default() -> Self {
        Self {
            keys: Interner::default(),
            position: Vec::new(),
            step: Vec::new(),
            steps: Vec::new(),
            edges: Vec::new(),
            terminal_edges: Vec::new(),
            guard_refs: Vec::new(),
            coefficients: Interner::default(),
            guards: Interner::default(),
            terminals: Interner::default(),
        }
    }
}

impl<K: Clone + Hash + Eq, P> Log<K, P> {
    fn key(&mut self, key: &K) -> u32 {
        let (id, new) = self.keys.intern_ref(key);
        if new {
            self.position.push(None);
            self.step.push(NONE);
        }
        id
    }

    fn coefficient(
        &mut self,
        coefficient: &Coefficient,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<u32, ReductionError> {
        let (id, new) = self.coefficients.intern_ref(coefficient);
        if new {
            // Values are combined with validated arithmetic.
            context
                .validate_with_limits(coefficient, limits)
                .map_err(ReductionError::from)?;
        }
        Ok(id)
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        key: &K,
        position: OrderedPosition<P>,
        step: &OrderedStep<K>,
        forward: bool,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<u32, ReductionError> {
        let id = self.key(key);
        if self.step[id as usize] != NONE {
            return Err(invariant("an ordered tail identity was expanded twice"));
        }
        self.position[id as usize] = Some(position);
        let edges = self.edges.len() as u32;
        for (child, coefficient) in &step.terms {
            if coefficient.is_zero() {
                continue;
            }
            let child = self.key(child);
            let coefficient = self.coefficient(coefficient, context, limits)?;
            self.edges.push((child, coefficient));
        }
        let terminals = self.terminal_edges.len() as u32;
        for (terminal, coefficient) in &step.terminals {
            if coefficient.is_zero() {
                continue;
            }
            let terminal = self.terminals.intern_ref(terminal).0;
            let coefficient = self.coefficient(coefficient, context, limits)?;
            self.terminal_edges.push((terminal, coefficient));
        }
        let guards = self.guard_refs.len() as u32;
        for guard in &step.nonzero_conditions {
            let guard = self.guards.intern_ref(guard).0;
            self.guard_refs.push(guard);
        }
        self.step[id as usize] = self.steps.len() as u32;
        self.steps.push(LoggedStep {
            edges: (edges, self.edges.len() as u32),
            terminals: (terminals, self.terminal_edges.len() as u32),
            guards: (guards, self.guard_refs.len() as u32),
            forward,
        });
        Ok(id)
    }
}

/// Encode `key` into `buffer`; `false` when it has no compact form.
fn encode<K: OrderedKernel>(buffer: &mut Vec<u8>, key: &K::Key) -> bool {
    buffer.clear();
    K::tail_encode(key, buffer)
}

/// The recorded state at the start of the candidate tail.
struct Checkpoint<Key> {
    phase: usize,
    ops: usize,
    /// Pending keys in pop order with their recorded slots.
    entries: Vec<(Key, u32)>,
    /// Terminal accumulators live at the checkpoint.
    terminals: Vec<(IntegralKey, u32)>,
    /// Guards retained at the checkpoint (a prefix of the final list).
    guards: usize,
}

/// The tail part of a recorded program to replace (see
/// [`OrderedWeights::rewrite_tail`]): truncate the program to `ops`
/// operations and add `T += w(slot) * u` for every entry.
pub(crate) struct TailRewrite<'a> {
    pub ops: usize,
    pub entries: &'a [(u32, &'a Coefficient)],
    /// The terminal's accumulator slot if it is live at the checkpoint.
    pub live: Option<u32>,
    /// The forward decision on the terminal: kept (else erased).
    pub keep: bool,
}

/// Shape of a rewritten tail program.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TailRewriteReport {
    /// Distinct value denominators, and products by `d^j / D` entering the
    /// terminal.
    pub denominators: usize,
    pub products: usize,
}

/// Counters of one request's tail, reported as a `{"stage":"tail",...}`
/// line of the modular statistics file.
#[derive(Default)]
struct TailStatistics {
    mode: &'static str,
    start_phase: Option<usize>,
    closed_phase: Option<usize>,
    closed_keys: usize,
    entry_keys: usize,
    logged_steps: usize,
    logged_edges: usize,
    closure_steps: usize,
    table_values: usize,
    computed_values: usize,
    parallel_values: usize,
    /// Dependency levels of the evaluated phases (same-phase children).
    value_levels: usize,
    added_entries: usize,
    replaced_ops: usize,
    added_ops: usize,
    denominators: usize,
    products: usize,
    resets: usize,
    checkpoints: usize,
    coverage_misses: usize,
    guard_misses: usize,
    /// Probe closing: keys decided nonzero, carriers of new guards.
    swept_keys: usize,
    carriers: usize,
    /// Reached keys decided zero that carry, or reach through keys that are
    /// not expanded, a guard not retained at the start of the tail.
    sensitive_keys: usize,
    /// Closings given up because a frontier weight or a contribution had an
    /// all-zero probe.
    zero_probes: usize,
    aborted: Option<&'static str>,
    closure_seconds: f64,
    values_seconds: f64,
    insert_seconds: f64,
    close_seconds: f64,
}

/// The outcome of one request's tail (tests only).
#[cfg(test)]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TailReport {
    pub mode: &'static str,
    pub start_phase: Option<usize>,
    pub closed_phase: Option<usize>,
    pub closed_keys: usize,
    pub entry_keys: usize,
    pub closure_steps: usize,
    pub table_values: usize,
    pub added_entries: usize,
    pub guard_misses: usize,
    pub swept_keys: usize,
    pub carriers: usize,
    pub sensitive_keys: usize,
    pub zero_probes: usize,
    pub aborted: Option<&'static str>,
}

#[cfg(test)]
thread_local! {
    static REPORTED: std::cell::RefCell<Vec<TailReport>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Tail outcomes reported on the current thread since the last call.
#[cfg(test)]
pub(crate) fn take_reported() -> Vec<TailReport> {
    REPORTED.with(|reported| std::mem::take(&mut *reported.borrow_mut()))
}

impl TailStatistics {
    fn report(&self) {
        #[cfg(test)]
        REPORTED.with(|reported| {
            reported.borrow_mut().push(TailReport {
                mode: self.mode,
                start_phase: self.start_phase,
                closed_phase: self.closed_phase,
                closed_keys: self.closed_keys,
                entry_keys: self.entry_keys,
                closure_steps: self.closure_steps,
                table_values: self.table_values,
                added_entries: self.added_entries,
                guard_misses: self.guard_misses,
                swept_keys: self.swept_keys,
                carriers: self.carriers,
                sensitive_keys: self.sensitive_keys,
                zero_probes: self.zero_probes,
                aborted: self.aborted,
            })
        });
        let text = |value: Option<usize>| value.map_or("null".to_owned(), |v| v.to_string());
        super::modular::report_line(&format!(
            concat!(
                "{{\"stage\":\"tail\",\"mode\":\"{}\",\"start_phase\":{},\"closed_phase\":{},",
                "\"closed_keys\":{},\"entry_keys\":{},\"logged_steps\":{},\"logged_edges\":{},",
                "\"closure_steps\":{},\"table_values\":{},\"computed_values\":{},",
                "\"parallel_values\":{},\"value_levels\":{},\"added_entries\":{},",
                "\"replaced_ops\":{},\"added_ops\":{},",
                "\"denominators\":{},\"products\":{},",
                "\"resets\":{},\"checkpoints\":{},\"coverage_misses\":{},\"guard_misses\":{},",
                "\"swept_keys\":{},\"carriers\":{},\"sensitive_keys\":{},\"zero_probes\":{},",
                "\"aborted\":{},\"closure_seconds\":{:.6},\"values_seconds\":{:.6},",
                "\"insert_seconds\":{:.6},\"close_seconds\":{:.6}}}"
            ),
            self.mode,
            text(self.start_phase),
            text(self.closed_phase),
            self.closed_keys,
            self.entry_keys,
            self.logged_steps,
            self.logged_edges,
            self.closure_steps,
            self.table_values,
            self.computed_values,
            self.parallel_values,
            self.value_levels,
            self.added_entries,
            self.replaced_ops,
            self.added_ops,
            self.denominators,
            self.products,
            self.resets,
            self.checkpoints,
            self.coverage_misses,
            self.guard_misses,
            self.swept_keys,
            self.carriers,
            self.sensitive_keys,
            self.zero_probes,
            self.aborted
                .map_or("null".to_owned(), |reason| format!("\"{reason}\"")),
            self.closure_seconds,
            self.values_seconds,
            self.insert_seconds,
            self.close_seconds,
        ));
    }
}

/// The adjoint tail of one request (see the module docs).
pub(super) struct TailRun<K: OrderedKernel> {
    table: Option<Arc<OrderedTailTable>>,
    /// Log the candidate tail's steps (to rewrite or to populate).
    log_enabled: bool,
    /// The weights record a program that can be rewritten.
    rewrite: bool,
    threads: usize,
    start: usize,
    terminal: Option<u32>,
    conflict: bool,
    closed: bool,
    log: Log<K::Key, K::Priority>,
    checkpoint: Option<Checkpoint<K::Key>>,
    statistics: TailStatistics,
    buffer: Vec<u8>,
}

impl<K: OrderedKernel> TailRun<K> {
    /// The tail of a request with weights `weights`, or `None` when it can
    /// change nothing (exact weights without a table).
    pub(super) fn new<A: OrderedWeights>(
        kernel: &mut K,
        weights: &A,
        threads: usize,
        logging: bool,
    ) -> Result<Option<Self>, K::Error> {
        // Diagnostic limits are checked at the exact point of the forward
        // execution; completing a tail takes steps the forward pass never
        // takes.
        if kernel.limits().diagnostics_enabled {
            return Ok(None);
        }
        let table = kernel.tail_table()?;
        let rewrite = logging && weights.tail_checkpoint().is_some();
        let populate = logging && table.as_ref().is_some_and(|table| table.populates());
        if !rewrite && !populate && table.is_none() {
            return Ok(None);
        }
        Ok(Some(Self {
            table,
            log_enabled: rewrite || populate,
            rewrite,
            threads: threads.max(1),
            start: 0,
            terminal: None,
            conflict: false,
            closed: false,
            log: Log::default(),
            checkpoint: None,
            statistics: TailStatistics {
                mode: "forward",
                ..Default::default()
            },
            buffer: Vec::new(),
        }))
    }

    fn reset(&mut self, start: usize) {
        self.start = start;
        self.terminal = None;
        self.conflict = false;
        self.log = Log::default();
        self.checkpoint = None;
        self.statistics.resets += 1;
    }

    /// At the start of `phase`, before anything of it is recorded: close the
    /// tail from the table when that leaves the guards unchanged (`true`),
    /// else take the checkpoint of a candidate tail starting here.
    pub(super) fn phase_started<A: OrderedWeights>(
        &mut self,
        kernel: &mut K,
        weights: &mut A,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
        state: &mut Admitted<K, A>,
        phase: usize,
    ) -> Result<bool, K::Error> {
        if self.closed {
            return Ok(false);
        }
        if self.try_close(kernel, weights, request, work, state, phase)? {
            return Ok(true);
        }
        if self.rewrite
            && phase == self.start
            && self.checkpoint.as_ref().is_none_or(|c| c.phase != phase)
        {
            let ops = weights
                .tail_checkpoint()
                .ok_or_else(|| invariant("recorded weights lost their checkpoint"))?;
            let slot = |weight: &A::Weight| {
                A::tail_slot(weight).ok_or_else(|| invariant("recorded weight without a slot"))
            };
            let mut entries = Vec::with_capacity(state.frontier.len());
            for (scheduled, weight) in state.frontier.iter() {
                entries.push((scheduled.key.clone(), slot(weight)?));
            }
            let mut terminals = Vec::with_capacity(state.output.len());
            for (key, weight) in &state.output {
                terminals.push((key.clone(), slot(weight)?));
            }
            self.checkpoint = Some(Checkpoint {
                phase,
                ops,
                entries,
                terminals,
                guards: state.nonzero_conditions.len(),
            });
            self.statistics.checkpoints += 1;
        }
        Ok(false)
    }

    /// Close the tail from the table: every pending key is tabulated with
    /// one common terminal, and either every guard of every tabulated
    /// reduction is already retained, or the arithmetic decides erasures by
    /// probes and the forward pass's decisions are repeated over the
    /// tabulated steps (see the module docs).
    fn try_close<A: OrderedWeights>(
        &mut self,
        kernel: &mut K,
        weights: &mut A,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
        state: &mut Admitted<K, A>,
        phase: usize,
    ) -> Result<bool, K::Error> {
        let Some(table) = self.table.clone() else {
            return Ok(false);
        };
        if state.frontier.is_empty() {
            return Ok(false);
        }
        let started = Instant::now();
        let data = table.read();
        if data.entries.is_empty() {
            return Ok(false);
        }
        // 1. Coverage, in pop order.
        let mut ids = Vec::with_capacity(state.frontier.len());
        let mut terminal = None;
        for (scheduled, _) in state.frontier.iter() {
            let id = if encode::<K>(&mut self.buffer, &scheduled.key) {
                data.id(&self.buffer)
            } else {
                None
            };
            let Some(id) = id else {
                if !ids.is_empty() {
                    self.statistics.coverage_misses += 1;
                }
                return Ok(false);
            };
            if let Some(t) = data.entry(id).terminal {
                match terminal {
                    None => terminal = Some(t),
                    Some(existing) if existing != t => return Ok(false),
                    _ => {}
                }
            }
            ids.push(id);
        }
        // 2. Guards.
        let mut retained: Vec<Option<bool>> = vec![None; data.guards.len()];
        let mut is_retained = |guard: u32| {
            *retained[guard as usize].get_or_insert_with(|| {
                state
                    .guards
                    .find(&state.nonzero_conditions, data.guards.value(guard))
                    .is_some()
            })
        };
        let sets: BTreeSet<u32> = ids.iter().map(|&id| data.entry(id).guards).collect();
        let unchanged = sets
            .iter()
            .all(|&set| data.sets.value(set).iter().all(|&guard| is_retained(guard)));
        let mut carriers = Vec::new();
        let mut swept = None;
        if !unchanged {
            self.statistics.guard_misses += 1;
            let mut probes = Vec::with_capacity(ids.len());
            for (_, weight) in state.frontier.iter() {
                let Some(probe) = weights.tail_probe(weight) else {
                    // Exact decisions cannot be repeated from the table.
                    return Ok(false);
                };
                if weights.tail_probe_is_zero(&probe) {
                    // The forward pass pops a pending key without a zero
                    // test; a vanishing probe would read as erased here.
                    self.statistics.zero_probes += 1;
                    return Ok(false);
                }
                probes.push(probe);
            }
            // The forward decisions over the tabulated steps.
            let mut merged: Vec<Option<A::Probe>> = vec![None; data.entries.len()];
            for (&id, &probe) in ids.iter().zip(&probes) {
                merged[id as usize] = Some(probe);
            }
            let mut coefficient_probes: Vec<Option<A::Probe>> = vec![None; data.coefficients.len()];
            let (mut popped, mut emitted) = (0usize, 0usize);
            let mut expanded = vec![false; data.entries.len()];
            let mut erased = Vec::new();
            for &id in &data.order {
                let Some(probe) = merged[id as usize] else {
                    continue;
                };
                let entry = *data.entry(id);
                if weights.tail_probe_is_zero(&probe) {
                    // Erased: checked below (with what only it reaches).
                    erased.push(id);
                    continue;
                }
                popped += 1;
                expanded[id as usize] = true;
                if data
                    .own_guards(&entry)
                    .iter()
                    .any(|&guard| !is_retained(guard))
                {
                    carriers.push(id);
                }
                for &(child, coefficient) in data.children(&entry) {
                    let factor = match coefficient_probes[coefficient as usize] {
                        Some(factor) => factor,
                        None => {
                            let factor = weights
                                .tail_probe_coefficient(data.coefficients.value(coefficient))?;
                            coefficient_probes[coefficient as usize] = Some(factor);
                            factor
                        }
                    };
                    // The forward pass inserts every contribution with a
                    // nonzero exact coefficient; one whose probe vanishes
                    // would be invisible to the probe sums here.
                    let mut contribution = None;
                    weights.tail_probe_add_product(&mut contribution, &probe, &factor);
                    if contribution.is_none_or(|value| weights.tail_probe_is_zero(&value)) {
                        self.statistics.zero_probes += 1;
                        return Ok(false);
                    }
                    weights.tail_probe_add_product(&mut merged[child as usize], &probe, &factor);
                    emitted += 1;
                }
                if entry.has_terminal_emission() {
                    emitted += 1;
                }
            }
            // An erased key is certified by no replay once the tail is
            // closed: if it, or a key reachable from it through keys the
            // sweep does not expand, carries a guard that is not retained,
            // its erasure could change the guards: expand the tail forward.
            let mut fresh_below: Vec<Option<bool>> = vec![None; data.entries.len()];
            let mut sensitive = 0usize;
            for &id in &erased {
                let mut stack = vec![(id, false)];
                while let Some((node, children_done)) = stack.pop() {
                    if fresh_below[node as usize].is_some() {
                        continue;
                    }
                    let entry = *data.entry(node);
                    let below = data
                        .children(&entry)
                        .iter()
                        .map(|&(child, _)| child)
                        .filter(|&child| !expanded[child as usize]);
                    if !children_done {
                        stack.push((node, true));
                        stack.extend(
                            below
                                .filter(|&child| fresh_below[child as usize].is_none())
                                .map(|child| (child, false)),
                        );
                        continue;
                    }
                    let fresh = data
                        .own_guards(&entry)
                        .iter()
                        .any(|&guard| !is_retained(guard))
                        || data
                            .children(&entry)
                            .iter()
                            .filter(|&&(child, _)| !expanded[child as usize])
                            .any(|&(child, _)| fresh_below[child as usize] == Some(true));
                    fresh_below[node as usize] = Some(fresh);
                }
                if fresh_below[id as usize] == Some(true) {
                    sensitive += 1;
                }
            }
            if sensitive > 0 {
                self.statistics.sensitive_keys += sensitive;
                return Ok(false);
            }
            swept = Some((popped, emitted));
        }
        // 3. Guards of the expanded carriers, in pop order.
        let mut ordered = Vec::with_capacity(carriers.len());
        for &id in &carriers {
            let Some(key) = kernel.tail_key(&data.keys[id as usize]) else {
                return Ok(false);
            };
            let position = kernel.position(&key)?;
            ordered.push((Reverse(position.phase), position.priority, id));
        }
        // Pop order: the greatest scheduled position first.
        ordered.sort_by(|left, right| right.cmp(left));
        let new_guards: Vec<Coefficient> = ordered
            .iter()
            .flat_map(|&(_, _, id)| data.own_guards(data.entry(id)).iter().copied())
            .map(|guard| data.guards.value(guard).clone())
            .collect();
        let terminal = terminal.map(|id| data.terminals.value(id).clone());
        let values: Vec<Coefficient> = ids
            .iter()
            .map(|&id| data.values.value(data.entry(id).value).clone())
            .collect();
        drop(data);
        for guard in &new_guards {
            state
                .guards
                .intern(&mut state.nonzero_conditions, guard, |_| Ok(()))?;
        }
        let keys: Vec<K::Key> = state
            .frontier
            .iter()
            .map(|(scheduled, _)| scheduled.key.clone())
            .collect();
        // 4. T += sum_K w(K) u(K).
        let limits = kernel.limits();
        let mut closed_emissions = 0usize;
        let grouped = weights.tail_checkpoint().is_some()
            && state
                .frontier
                .iter()
                .next()
                .is_some_and(|(_, weight)| weights.tail_probe(weight).is_some());
        if grouped {
            // Recorded probe arithmetic: one grouped rewrite (no
            // truncation); the terminal's fate is the forward pass's probe
            // decision on the same linear functional.
            let mut sum = terminal
                .as_ref()
                .and_then(|terminal| state.output.get(terminal))
                .and_then(|weight| weights.tail_probe(weight));
            let mut entries = Vec::with_capacity(keys.len());
            for (key, value) in keys.iter().zip(&values) {
                let pending = state
                    .frontier
                    .pending
                    .remove(key)
                    .ok_or_else(|| invariant("a pending tail key vanished"))?;
                request.release_pending_frame()?;
                replace_monomials(
                    &mut state.progress.frontier_coefficient_monomials,
                    weights.monomials(&pending.weight)?,
                    0,
                )?;
                let slot = A::tail_slot(&pending.weight)
                    .ok_or_else(|| invariant("recorded weight without a slot"))?;
                if !value.is_zero() {
                    let probe = weights
                        .tail_probe(&pending.weight)
                        .ok_or_else(|| invariant("recorded weight without a probe"))?;
                    let factor = weights.tail_probe_coefficient(value)?;
                    weights.tail_probe_add_product(&mut sum, &probe, &factor);
                    closed_emissions += 1;
                }
                entries.push((slot, value));
            }
            let keep = sum.is_some_and(|sum| !weights.tail_probe_is_zero(&sum));
            let mut output = terminal
                .as_ref()
                .and_then(|terminal| state.output.remove(terminal));
            let live = output.as_ref().and_then(|weight| A::tail_slot(weight));
            let ops = weights
                .tail_checkpoint()
                .ok_or_else(|| invariant("recorded weights lost their checkpoint"))?;
            if terminal.is_none() && keep {
                return Err(invariant("a kept tail terminal without key").into());
            }
            let report = weights.rewrite_tail(
                TailRewrite {
                    ops,
                    entries: &entries,
                    live,
                    keep,
                },
                &mut output,
                sum,
            )?;
            self.statistics.denominators = report.denominators;
            self.statistics.products = report.products;
            if let (Some(terminal), Some(weight)) = (terminal.clone(), output) {
                state.output.insert(terminal, weight);
            }
            let progress = &mut state.progress;
            if keep {
                progress.terminal_merges = progress.terminal_merges.saturating_add(1);
            }
            progress.terminal_coefficient_monomials = state.output.len();
            progress.peak_terminal_coefficient_monomials = progress
                .peak_terminal_coefficient_monomials
                .max(progress.terminal_coefficient_monomials);
        } else {
            // Value arithmetics: T += w(K) u(K), in pop order, as terminal
            // emissions.
            let progress = &mut state.progress;
            for (key, value) in keys.iter().zip(values) {
                let pending = state
                    .frontier
                    .pending
                    .remove(key)
                    .ok_or_else(|| invariant("a pending tail key vanished"))?;
                request.release_pending_frame()?;
                replace_monomials(
                    &mut progress.frontier_coefficient_monomials,
                    weights.monomials(&pending.weight)?,
                    0,
                )?;
                let weight = pending.weight;
                weights.begin_step(&weight)?;
                if !value.is_zero() {
                    let terminal = terminal
                        .as_ref()
                        .ok_or_else(|| invariant("a nonzero tail value without terminal"))?;
                    let contribution = weights.scale(&weight, &value)?;
                    if !weights.contribution_is_zero(&contribution) {
                        closed_emissions += 1;
                        merge_terminal_contribution(
                            weights,
                            &mut state.output,
                            progress,
                            work,
                            request,
                            limits,
                            terminal.clone(),
                            contribution,
                        )?;
                    }
                }
                weights.end_step(weight);
            }
        }
        let progress = &mut state.progress;
        let (processed, emitted) = swept.unwrap_or((keys.len(), closed_emissions));
        progress.processed_keys = progress.processed_keys.saturating_add(processed);
        progress.emitted_terms = progress.emitted_terms.saturating_add(emitted);
        state.frontier.order.clear();
        progress.frontier_keys = 0;
        progress.terminal_keys = state.output.len();
        kernel.observe(progress, None)?;
        self.closed = true;
        self.statistics.mode = if swept.is_some() {
            "closed-probes"
        } else {
            "closed"
        };
        self.statistics.closed_phase = Some(phase);
        self.statistics.closed_keys = keys.len();
        self.statistics.swept_keys = processed;
        self.statistics.carriers = carriers.len();
        self.statistics.close_seconds = started.elapsed().as_secs_f64();
        Ok(true)
    }

    /// Log one expanded step of the candidate tail.
    /// Whether [`Self::observe_step`] records anything now.
    pub(super) fn logging(&self) -> bool {
        !self.closed && self.log_enabled && !self.conflict
    }

    pub(super) fn observe_step(
        &mut self,
        node: &Scheduled<K::Key, K::Priority>,
        step: &OrderedStep<K::Key>,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ReductionError> {
        if self.closed || !self.log_enabled || self.conflict {
            return Ok(());
        }
        for (terminal, coefficient) in &step.terminals {
            if coefficient.is_zero() {
                continue;
            }
            let id = self.log.terminals.intern_ref(terminal).0;
            match self.terminal {
                None => self.terminal = Some(id),
                Some(existing) if existing != id => {
                    // A second terminal: no tail starts at or before this
                    // phase.
                    self.conflict = true;
                    return Ok(());
                }
                _ => {}
            }
        }
        self.log.record(
            &node.key,
            OrderedPosition {
                phase: node.phase.0,
                priority: node.priority.clone(),
            },
            step,
            true,
            context,
            limits,
        )?;
        Ok(())
    }

    /// At the end of `phase`: a phase that emitted a second terminal moves
    /// the candidate tail behind it.
    pub(super) fn phase_finished(&mut self, phase: usize) {
        if self.conflict {
            self.reset(phase + 1);
        }
    }

    /// After the last phase: complete, evaluate and tabulate the candidate
    /// tail, and rewrite the recorded program of recorded weights.
    pub(super) fn finish<A: OrderedWeights>(
        mut self,
        kernel: &mut K,
        weights: &mut A,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
        state: &mut Admitted<K, A>,
    ) -> Result<(), K::Error> {
        let result = self.evaluate(kernel, weights, request, work, state);
        self.statistics.report();
        result
    }

    fn abort(&mut self, reason: &'static str) -> Result<(), K::Error> {
        self.statistics.aborted = Some(reason);
        Ok(())
    }

    fn evaluate<A: OrderedWeights>(
        &mut self,
        kernel: &mut K,
        weights: &mut A,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
        state: &mut Admitted<K, A>,
    ) -> Result<(), K::Error> {
        if self.closed || !self.log_enabled || self.log.steps.is_empty() {
            return Ok(());
        }
        self.statistics.start_phase = Some(self.start);
        self.statistics.logged_steps = self.log.steps.len();
        self.statistics.logged_edges = self.log.edges.len();
        let context = kernel.context().clone();
        let limits = kernel.limits().exact_algebra;
        let table = self.table.clone();

        // 1. Complete the tail: every reached identity needs a value, from
        //    its logged step, the table, or a step computed now (keys the
        //    forward pass reached and erased).
        let started = Instant::now();
        let mut tabulated: HashMap<u32, Tabulated> = HashMap::new();
        let mut worklist: Vec<u32> = self
            .log
            .edges
            .iter()
            .map(|&(child, _)| child)
            .filter(|&child| self.log.step[child as usize] == NONE)
            .collect();
        if let Some(checkpoint) = &self.checkpoint {
            for (key, _) in &checkpoint.entries {
                let id = self.log.key(key);
                if self.log.step[id as usize] == NONE {
                    worklist.push(id);
                }
            }
        }
        // Tabulated keys need no evaluation (nor a step).
        let lookup = |log: &Log<K::Key, K::Priority>,
                      buffer: &mut Vec<u8>,
                      tabulated: &mut HashMap<u32, Tabulated>,
                      id: u32| {
            let Some(table) = &table else {
                return false;
            };
            if tabulated.contains_key(&id) {
                return true;
            }
            if !encode::<K>(buffer, log.keys.value(id)) {
                return false;
            }
            let data = table.read();
            let Some(entry_id) = data.id(buffer) else {
                return false;
            };
            let entry = *data.entry(entry_id);
            tabulated.insert(
                id,
                Tabulated {
                    entry: entry_id,
                    value: data.values.value(entry.value).clone(),
                    terminal: entry.terminal.map(|t| data.terminals.value(t).clone()),
                    guards: data
                        .sets
                        .value(entry.guards)
                        .iter()
                        .map(|&guard| data.guards.value(guard).clone())
                        .collect(),
                },
            );
            true
        };
        if table
            .as_ref()
            .is_some_and(|table| !table.read().entries.is_empty())
        {
            for id in 0..self.log.keys.len() as u32 {
                lookup(&self.log, &mut self.buffer, &mut tabulated, id);
            }
        }
        while let Some(id) = worklist.pop() {
            if self.log.step[id as usize] != NONE
                || lookup(&self.log, &mut self.buffer, &mut tabulated, id)
            {
                continue;
            }
            let key = self.log.keys.value(id).clone();
            let position = kernel.position(&key)?;
            // The forward pass never expanded this key; if its step fails,
            // the tail keeps its forward program instead of failing the
            // request.
            let Ok(step) = kernel.step(&key, request, work) else {
                return self.abort("the step of an erased key failed");
            };
            for (terminal, coefficient) in &step.terminals {
                if coefficient.is_zero() {
                    continue;
                }
                let terminal = self.log.terminals.intern_ref(terminal).0;
                match self.terminal {
                    None => self.terminal = Some(terminal),
                    Some(existing) if existing != terminal => {
                        return self.abort("a reached identity reduces to another terminal");
                    }
                    _ => {}
                }
            }
            self.log
                .record(&key, position, &step, false, &context, limits)?;
            self.statistics.closure_steps += 1;
            for (child, coefficient) in &step.terms {
                if coefficient.is_zero() {
                    continue;
                }
                let child = self.log.key(child);
                if self.log.step[child as usize] == NONE {
                    worklist.push(child);
                }
            }
        }
        let terminal_key = self
            .terminal
            .map(|terminal| self.log.terminals.value(terminal).clone());
        for tabulated in tabulated.values() {
            if tabulated.terminal.is_some() && tabulated.terminal != terminal_key {
                return self.abort("a tabulated identity reduces to another terminal");
            }
        }
        self.statistics.closure_seconds = started.elapsed().as_secs_f64();

        // 2. Values u(K) and guard sets, children first.
        let started = Instant::now();
        let count = self.log.keys.len();
        let mut values: Vec<Option<Coefficient>> = vec![None; count];
        let mut set_of: Vec<u32> = vec![NONE; count];
        let mut sets = GuardSets::default();
        for (&id, tabulated) in &tabulated {
            values[id as usize] = Some(tabulated.value.clone());
            let ids: Vec<u32> = tabulated
                .guards
                .iter()
                .map(|guard| self.log.guards.intern_ref(guard).0)
                .collect();
            set_of[id as usize] = sets.intern(GuardSets::of(ids));
        }
        self.statistics.table_values = tabulated.len();
        let mut order: Vec<u32> = (0..count as u32)
            .filter(|&id| self.log.step[id as usize] != NONE && !tabulated.contains_key(&id))
            .collect();
        for &id in &order {
            if self.log.position[id as usize].is_none() {
                return Err(invariant("a logged tail identity has no position").into());
            }
        }
        // Children strictly precede their parents in ascending scheduler
        // order (later phases first, then increasing priority).
        order.sort_by(|&left, &right| {
            let (left, right) = (
                self.log.position[left as usize].as_ref().expect("position"),
                self.log.position[right as usize]
                    .as_ref()
                    .expect("position"),
            );
            right
                .phase
                .cmp(&left.phase)
                .then_with(|| left.priority.cmp(&right.priority))
        });
        let workers = if self.threads > 1 {
            Some(Workers::new(self.threads)?)
        } else {
            None
        };
        let mut level: Vec<u32> = vec![0; count];
        let mut index = 0;
        while index < order.len() {
            let phase = self.log.position[order[index] as usize]
                .as_ref()
                .expect("position")
                .phase;
            let mut end = index;
            while end < order.len()
                && self.log.position[order[end] as usize]
                    .as_ref()
                    .expect("position")
                    .phase
                    == phase
            {
                end += 1;
            }
            let group = &order[index..end];
            // Steps may emit children of their own phase (owner phases;
            // layered Route phases through partial keys). Level 0 has no
            // unevaluated child, level l only children of levels < l
            // (children precede parents in `group`), so the values of one
            // level are independent and evaluated in parallel.
            let mut levels: Vec<Vec<u32>> = Vec::new();
            for &id in group {
                let step = &self.log.steps[self.log.step[id as usize] as usize];
                let own = self.log.edges[step.edges.0 as usize..step.edges.1 as usize]
                    .iter()
                    .filter(|&&(child, _)| values[child as usize].is_none())
                    .map(|&(child, _)| level[child as usize] + 1)
                    .max()
                    .unwrap_or(0);
                level[id as usize] = own;
                let own = own as usize;
                if levels.len() <= own {
                    levels.resize_with(own + 1, Vec::new);
                }
                levels[own].push(id);
            }
            self.statistics.value_levels += levels.len();
            let graph = Graph {
                step: &self.log.step,
                steps: &self.log.steps,
                edges: &self.log.edges,
                terminal_edges: &self.log.terminal_edges,
                coefficients: self.log.coefficients.values(),
            };
            let evaluate = |id: u32, values: &[Option<Coefficient>]| {
                graph.combine(id, values, &context, limits)
            };
            for ids in &levels {
                match &workers {
                    Some(workers) if ids.len() >= PARALLEL_LEVEL => {
                        let mut items: Vec<(u32, Option<Result<Coefficient, ReductionError>>)> =
                            ids.iter().map(|&id| (id, None)).collect();
                        let shared = &values;
                        workers.for_each(&mut items, |item| {
                            item.1 = Some(evaluate(item.0, shared));
                        });
                        for (id, value) in items {
                            values[id as usize] = Some(value.expect("evaluated")?);
                        }
                        self.statistics.parallel_values += ids.len();
                    }
                    _ => {
                        for &id in ids {
                            let value = evaluate(id, &values)?;
                            values[id as usize] = Some(value);
                        }
                    }
                }
            }
            for &id in group {
                let step = &self.log.steps[self.log.step[id as usize] as usize];
                let mut words = GuardSets::of(
                    self.log.guard_refs[step.guards.0 as usize..step.guards.1 as usize]
                        .iter()
                        .copied(),
                );
                let mut seen = BTreeSet::new();
                for &(child, _) in &self.log.edges[step.edges.0 as usize..step.edges.1 as usize] {
                    let set = set_of[child as usize];
                    if set == NONE {
                        return Err(invariant("a tail child has no guard set").into());
                    }
                    if seen.insert(set) {
                        GuardSets::union_into(&mut words, sets.sets.value(set));
                    }
                }
                set_of[id as usize] = sets.intern(words);
            }
            self.statistics.computed_values += group.len();
            index = end;
        }
        drop(workers);
        self.statistics.values_seconds = started.elapsed().as_secs_f64();

        // 3. Tabulate the new values with their steps.
        if let Some(table) = table.as_ref().filter(|table| table.populates()) {
            let started = Instant::now();
            let mut new = Vec::with_capacity(order.len());
            let mut encodable = true;
            for &id in &order {
                if encode::<K>(&mut self.buffer, self.log.keys.value(id)) {
                    new.push((self.buffer.clone().into_boxed_slice(), id));
                } else {
                    encodable = false;
                    break;
                }
            }
            let log = &self.log;
            let terminal = terminal_key.as_ref();
            let added = if !encodable {
                0
            } else {
                table.update(|data| {
                    let mut entry_of: HashMap<u32, u32> = tabulated
                        .iter()
                        .map(|(&id, tabulated)| (id, tabulated.entry))
                        .collect();
                    let mut reserved = Vec::with_capacity(new.len());
                    for (key, id) in new {
                        if let Some(entry) = data.id(&key) {
                            entry_of.insert(id, entry);
                        } else {
                            let entry = data.reserve(key);
                            entry_of.insert(id, entry);
                            reserved.push((id, entry));
                        }
                    }
                    // This run's ids in the table's interning.
                    let coefficients: Vec<u32> = log
                        .coefficients
                        .values()
                        .iter()
                        .map(|coefficient| data.coefficients.intern_ref(coefficient).0)
                        .collect();
                    let guards: Vec<u32> = log
                        .guards
                        .values()
                        .iter()
                        .map(|guard| data.guards.intern_ref(guard).0)
                        .collect();
                    let terminal = terminal.map(|key| data.terminals.intern_ref(key).0);
                    let mut set_ids: HashMap<u32, u32> = HashMap::new();
                    let mut children = Vec::new();
                    let mut own = Vec::new();
                    for &(id, entry) in &reserved {
                        let step = &log.steps[log.step[id as usize] as usize];
                        children.clear();
                        children.extend(
                            log.edges[step.edges.0 as usize..step.edges.1 as usize]
                                .iter()
                                .map(|&(child, coefficient)| {
                                    (entry_of[&child], coefficients[coefficient as usize])
                                }),
                        );
                        own.clear();
                        own.extend(
                            log.guard_refs[step.guards.0 as usize..step.guards.1 as usize]
                                .iter()
                                .map(|&guard| guards[guard as usize]),
                        );
                        let local = set_of[id as usize];
                        let set = *set_ids.entry(local).or_insert_with(|| {
                            let mut ids: Vec<u32> = GuardSets::members(sets.sets.value(local))
                                .map(|guard| guards[guard as usize])
                                .collect();
                            ids.sort_unstable();
                            ids.dedup();
                            data.sets.intern_ref(&ids.into_boxed_slice()).0
                        });
                        let value = values[id as usize].as_ref().expect("evaluated");
                        let terminal_coefficient = log.terminal_edges
                            [step.terminals.0 as usize..step.terminals.1 as usize]
                            .first()
                            .map(|&(_, coefficient)| coefficients[coefficient as usize]);
                        data.fill(
                            entry,
                            table::NewEntry {
                                terminal: if value.is_zero() { None } else { terminal },
                                value,
                                guards: set,
                                phase: log.position[id as usize].as_ref().expect("position").phase
                                    as u32,
                                children: &children,
                                terminal_coefficient,
                                own_guards: &own,
                            },
                        );
                    }
                    reserved.len()
                })?
            };
            self.statistics.added_entries = added;
            self.statistics.insert_seconds = started.elapsed().as_secs_f64();
        }

        // 4. Replace the recorded tail by one product per entry weight.
        if !self.rewrite {
            return Ok(());
        }
        let Some(checkpoint) = self.checkpoint.take().filter(|c| c.phase == self.start) else {
            return self.abort("no checkpoint at the start of the tail");
        };
        // Reached keys the forward pass erased: their erasure checks go away
        // with the forward program, so they must not carry a guard that was
        // not retained at the start of the tail.
        let fresh = |guard: &Coefficient| {
            state
                .guards
                .find(&state.nonzero_conditions, guard)
                .is_none_or(|index| index >= checkpoint.guards)
        };
        // Reached by the forward pass: an entry, or a child of an expanded
        // step (keys reached only through erased keys never got a weight).
        let mut reached = vec![false; self.log.keys.len()];
        for (key, _) in &checkpoint.entries {
            if let Some(id) = self.log.keys.get(key) {
                reached[id as usize] = true;
            }
        }
        for step in self.log.steps.iter().filter(|step| step.forward) {
            for &(child, _) in &self.log.edges[step.edges.0 as usize..step.edges.1 as usize] {
                reached[child as usize] = true;
            }
        }
        // The guards that an erasure could change are those of the erased
        // key and of every key reachable from it through keys the forward
        // pass did not expand. A tabulated key is walked over the table's
        // steps: its closure set (all guards of its reduction) also holds
        // the guards of keys the forward pass expanded, which would hold
        // back rewrites the same run makes without the table (a table
        // populated by another request).
        let expanded = |id: u32| {
            let step = self.log.step[id as usize];
            step != NONE && self.log.steps[step as usize].forward
        };
        let data = table
            .as_ref()
            .filter(|_| !tabulated.is_empty())
            .map(|table| table.read());
        let mut walk = data
            .as_deref()
            .map(|data| TableWalk::new(data, &tabulated, expanded));
        let mut fresh_below: Vec<Option<bool>> = vec![None; self.log.keys.len()];
        let mut sensitive = 0usize;
        for id in 0..self.log.keys.len() as u32 {
            if !reached[id as usize] || expanded(id) {
                continue;
            }
            let mut stack = vec![(id, false)];
            while let Some((node, children_done)) = stack.pop() {
                if fresh_below[node as usize].is_some() {
                    continue;
                }
                let step = self.log.step[node as usize];
                if step == NONE {
                    let fresh = match (tabulated.get(&node), walk.as_mut()) {
                        (Some(tabulated), Some(walk)) => walk.fresh_below(tabulated.entry, &fresh),
                        (Some(tabulated), None) => tabulated.guards.iter().any(fresh),
                        (None, _) => false,
                    };
                    fresh_below[node as usize] = Some(fresh);
                    continue;
                }
                let logged = &self.log.steps[step as usize];
                let children = &self.log.edges[logged.edges.0 as usize..logged.edges.1 as usize];
                if !children_done {
                    stack.push((node, true));
                    stack.extend(
                        children
                            .iter()
                            .map(|&(child, _)| child)
                            .filter(|&child| {
                                !expanded(child) && fresh_below[child as usize].is_none()
                            })
                            .map(|child| (child, false)),
                    );
                    continue;
                }
                let own = self.log.guard_refs[logged.guards.0 as usize..logged.guards.1 as usize]
                    .iter()
                    .any(|&guard| fresh(self.log.guards.value(guard)));
                let below = children.iter().any(|&(child, _)| {
                    !expanded(child) && fresh_below[child as usize] == Some(true)
                });
                fresh_below[node as usize] = Some(own || below);
            }
            if fresh_below[id as usize] == Some(true) {
                sensitive += 1;
            }
        }
        drop(walk);
        drop(data);
        self.statistics.sensitive_keys += sensitive;
        if sensitive > 0 {
            return self.abort("an erased key reaches a guard new to the tail");
        }
        let mut entries = Vec::with_capacity(checkpoint.entries.len());
        for (key, slot) in &checkpoint.entries {
            let id = self
                .log
                .keys
                .get(key)
                .ok_or_else(|| invariant("a tail entry key was not logged"))?;
            let value = values[id as usize]
                .as_ref()
                .ok_or_else(|| invariant("a tail entry key has no value"))?;
            entries.push((*slot, value));
        }
        self.statistics.entry_keys = entries.len();
        let live = terminal_key.as_ref().and_then(|terminal| {
            checkpoint
                .terminals
                .iter()
                .find(|(key, _)| key == terminal)
                .map(|&(_, slot)| slot)
        });
        if terminal_key.is_none() && entries.iter().any(|(_, value)| !value.is_zero()) {
            return Err(invariant("nonzero tail values without a terminal").into());
        }
        let mut output = terminal_key
            .as_ref()
            .and_then(|terminal| state.output.remove(terminal));
        let keep = output.is_some();
        let before = weights.tail_checkpoint().unwrap_or(0);
        let report = weights.rewrite_tail(
            TailRewrite {
                ops: checkpoint.ops,
                entries: &entries,
                live,
                keep,
            },
            &mut output,
            None,
        )?;
        if let (Some(terminal), Some(weight)) = (terminal_key, output) {
            state.output.insert(terminal, weight);
        }
        let after = weights.tail_checkpoint().unwrap_or(0);
        self.statistics.denominators = report.denominators;
        self.statistics.products = report.products;
        self.statistics.replaced_ops = before - checkpoint.ops;
        self.statistics.added_ops = after - checkpoint.ops;
        self.statistics.mode = "adjoint";
        Ok(())
    }
}

/// A reached identity found in the table.
struct Tabulated {
    entry: u32,
    value: Coefficient,
    terminal: Option<IntegralKey>,
    /// All guards of its reduction.
    guards: Vec<Coefficient>,
}

/// The guards an erasure could change below tabulated identities (see
/// `TailRun::evaluate`), walked over the table's steps: an entry's own
/// guards and those of every entry reachable from it through entries of
/// keys the forward pass did not expand. An entry whose closure set holds
/// no fresh guard is not entered.
struct TableWalk<'a> {
    data: &'a TailData,
    /// Entries of keys the forward pass expanded (all of the run's keys
    /// with that entry).
    expanded: HashMap<u32, bool>,
    /// Per table guard: fresh (memo).
    fresh: Vec<Option<bool>>,
    /// Per entry: reaches a fresh guard (memo).
    below: Vec<Option<bool>>,
}

impl<'a> TableWalk<'a> {
    fn new(
        data: &'a TailData,
        tabulated: &HashMap<u32, Tabulated>,
        expanded: impl Fn(u32) -> bool,
    ) -> Self {
        let mut entries: HashMap<u32, bool> = HashMap::with_capacity(tabulated.len());
        for (&id, tabulated) in tabulated {
            *entries.entry(tabulated.entry).or_insert(true) &= expanded(id);
        }
        Self {
            data,
            expanded: entries,
            fresh: vec![None; data.guards.len()],
            below: vec![None; data.entries.len()],
        }
    }

    fn fresh_below(&mut self, root: u32, fresh: &impl Fn(&Coefficient) -> bool) -> bool {
        let Self {
            data,
            expanded,
            fresh: memo,
            below,
        } = self;
        let data = *data;
        let mut is_fresh = |guard: u32| {
            *memo[guard as usize].get_or_insert_with(|| fresh(data.guards.value(guard)))
        };
        let walked = |child: u32| expanded.get(&child) != Some(&true);
        let mut stack = vec![(root, false)];
        while let Some((node, children_done)) = stack.pop() {
            if below[node as usize].is_some() {
                continue;
            }
            let entry = *data.entry(node);
            if !children_done {
                if !data
                    .sets
                    .value(entry.guards)
                    .iter()
                    .any(|&guard| is_fresh(guard))
                {
                    below[node as usize] = Some(false);
                    continue;
                }
                stack.push((node, true));
                stack.extend(
                    data.children(&entry)
                        .iter()
                        .map(|&(child, _)| child)
                        .filter(|&child| walked(child) && below[child as usize].is_none())
                        .map(|child| (child, false)),
                );
                continue;
            }
            let fresh = data.own_guards(&entry).iter().any(|&guard| is_fresh(guard))
                || data
                    .children(&entry)
                    .iter()
                    .any(|&(child, _)| walked(child) && below[child as usize] == Some(true));
            below[node as usize] = Some(fresh);
        }
        below[root as usize] == Some(true)
    }
}

/// The key-independent part of the log: what evaluating a value reads.
struct Graph<'a> {
    step: &'a [u32],
    steps: &'a [LoggedStep],
    edges: &'a [(u32, u32)],
    terminal_edges: &'a [(u32, u32)],
    coefficients: &'a [Coefficient],
}

impl Graph<'_> {
    /// `u(K)` of a logged step from the values of its children.
    fn combine(
        &self,
        id: u32,
        values: &[Option<Coefficient>],
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Coefficient, ReductionError> {
        // Values are exact rational-function sums; the caller's resource
        // budgets (and so its cancellation) are checked between two of them,
        // also on the workers, which run inside the captured budgets.
        symbolica::utils::resources::checkpoint()?;
        let step = &self.steps[self.step[id as usize] as usize];
        let mut sum: Option<Coefficient> = None;
        let mut add = |term: Coefficient| -> Result<(), ReductionError> {
            sum = Some(match sum.take() {
                None => term,
                Some(sum) => context
                    .try_add_validated(&sum, &term, limits)
                    .map_err(ReductionError::from)?,
            });
            Ok(())
        };
        for &(child, coefficient) in &self.edges[step.edges.0 as usize..step.edges.1 as usize] {
            let value = values[child as usize]
                .as_ref()
                .ok_or_else(|| invariant("a tail child has no value"))?;
            if value.is_zero() {
                continue;
            }
            let coefficient = &self.coefficients[coefficient as usize];
            let term = if super::is_one(coefficient) {
                value.clone()
            } else {
                context
                    .try_mul_validated(coefficient, value, limits)
                    .map_err(ReductionError::from)?
            };
            add(term)?;
        }
        for &(_, coefficient) in
            &self.terminal_edges[step.terminals.0 as usize..step.terminals.1 as usize]
        {
            add(self.coefficients[coefficient as usize].clone())?;
        }
        Ok(sum.unwrap_or_else(|| context.zero()))
    }
}
