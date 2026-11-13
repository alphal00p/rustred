//! fwd-opt: TFORM-style layered execution of a recorded (modular) request.
//!
//! `RUSTRED_ORDERED_LAYERED=on` (default off). The sequential scheduler pops
//! one key at a time and merges its children into one frontier, so the
//! merge/record loop is serial. This mode runs the forward pass the way
//! TFORM runs a module, with a global merge per layer:
//!
//! 1. A *window* is the greatest `RUSTRED_ORDERED_LAYERED_WINDOW` pending keys
//!    of the phase (default 8192), closed under same-phase descent above its
//!    last key; the steps of the closure are computed on the workers (as
//!    the pipelined source does). Every parent of a window key is a window
//!    key or was expanded before (children strictly descend).
//! 2. The window keys are cut into *layers* by their depth in the window DAG
//!    (layer of a key = 1 + the greatest layer of its window parents): all
//!    contributions to a key of layer `L` come from layers `< L` or earlier
//!    windows, so the keys of one layer are expanded together, like the
//!    terms of one module.
//! 3. Per layer, the children of the expanded keys are bucketed by the hash
//!    of the destination key into [`SHARDS`] frontier shards (per parent
//!    chunk, then per shard: no shared lock in the hot loop), and every
//!    shard merges its contributions on its own worker in the fixed order
//!    (parent, child): found -> probe sum, zero -> cancellation; vacant ->
//!    new key with a slot of the shard (`slot = local * SHARDS + shard`).
//!    This is TFORM's sort-and-merge (`.sort`) of the module's output.
//! 4. A serial scan emits the trace in layer order, parent order, child
//!    order: one `Pop` per parent, then the recorded `New`/`Add`/`Cancel` of
//!    each child, then the terminals; coefficient references are numbered at
//!    their first occurrence in this sequence (`intern_id`).
//!
//! Every decision (windows, layers, shards, slots, the order of the merges
//! into one key, the trace) is a function of the request and the two
//! constants (window, shards) only, never of the thread count: the trace and
//! the output are identical for every thread count (one thread runs the same
//! algorithm on a one-thread pool). Against the sequential scheduler the
//! trace differs (slots, op order) but every key is expanded exactly once,
//! after all its contributions, with the same exact weight, so the terms
//! are the same.
//!
//! Not used with diagnostic limits, the adjoint tail, a cross-checked
//! arithmetic, or an adapter without a detached stepper (the sequential or
//! pipelined path runs instead).

use std::cmp::{Ordering, Reverse};
use std::collections::BTreeMap;
use std::hash::BuildHasher;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use hashbrown::HashTable;

use super::modular::{ModularRecorder, ModularWeight, Op, OpTag, Probe, ProbeField, SharedIntern};
use super::parallel::{ClosureStep, Workers, at_or_above, layered_closure};
use super::{ConcurrentOrderedKernel, OrderedOutput, TailOptions};
use super::{
    FrontierHash, FrontierKey, Lifter, OrderedFrontierEntry, OrderedKernel, OrderedPhaseBoundary,
    OrderedPhaseEvent, OrderedPosition, OrderedStateView, OrderedStep, OrderedStepper,
    OrderedWeightArithmetic, OrderedWeights, RawOutput, Scheduled, check_descent,
    check_descent_parts, frontier_entry, invariant, modular_output,
};

use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::{ReductionError, ReductionRequest, ReductionStatistics};
use crate::solver::CandidateOrderedReductionProgress;

/// Frontier shards (a constant: the trace must not depend on the thread
/// count).
const SHARDS: usize = 64;

/// Layered requests started in this process (keys the workers' handle caches).
static REQUESTS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// fwd-opt: this worker's light handles of the current request ->
    /// (shared-table id, probe values, zero).
    static HANDLES: std::cell::RefCell<(u64, std::collections::HashMap<u64, (u32, Probe, bool), FrontierHash>)> =
        std::cell::RefCell::new((u64::MAX, std::collections::HashMap::with_hasher(FrontierHash::Fx)));
}

/// `RUSTRED_ORDERED_LAYERED=on|off` (fwd-opt; default off).
pub(crate) fn environment() -> Option<bool> {
    static ON: std::sync::OnceLock<Option<bool>> = std::sync::OnceLock::new();
    *ON.get_or_init(|| super::switch_environment("RUSTRED_ORDERED_LAYERED"))
}

/// `RUSTRED_ORDERED_LAYERED_WINDOW=n`: keys per window (default 8192).
fn window_keys() -> usize {
    static N: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("RUSTRED_ORDERED_LAYERED_WINDOW")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(8192)
    })
}

/// `RUSTRED_ORDERED_LAYERED_CHECK=1`: verify the breadth-first certificate
/// of the layered execution (every key expanded once, phases never regress)
/// with a set of every expanded key (memory: one packed key per step).
fn check_requested() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| super::switch_environment("RUSTRED_ORDERED_LAYERED_CHECK") == Some(true))
}

#[inline]
fn shard_of(hash: u64) -> usize {
    (hash.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - 6)) as usize % SHARDS
}

/// One pending key of a shard.
struct Entry<Q, P> {
    packed: Q,
    hash: u64,
    position: OrderedPosition<P>,
    weight: ModularWeight,
    generation: u32,
    live: bool,
}

/// One frontier shard: pending keys (slab + hash table), per-phase lists of
/// slab indices (lazily cleaned), and the shard's slot allocator.
struct Shard<Q, P> {
    slab: Vec<Entry<Q, P>>,
    free_slab: Vec<u32>,
    table: HashTable<u32>,
    phases: Vec<Vec<(u32, u32)>>,
    free_slots: Vec<u32>,
    next_local: u32,
    live: usize,
}

impl<Q: Eq, P> Shard<Q, P> {
    fn new(phases: usize) -> Self {
        Self {
            slab: Vec::new(),
            free_slab: Vec::new(),
            table: HashTable::new(),
            phases: (0..phases).map(|_| Vec::new()).collect(),
            free_slots: Vec::new(),
            next_local: 0,
            live: 0,
        }
    }

    fn find(&self, hash: u64, packed: &Q) -> Option<u32> {
        let slab = &self.slab;
        self.table
            .find(hash, |&index| slab[index as usize].packed == *packed)
            .copied()
    }

    fn insert(
        &mut self,
        packed: Q,
        hash: u64,
        position: OrderedPosition<P>,
        weight: ModularWeight,
    ) {
        let phase = position.phase;
        let index = match self.free_slab.pop() {
            Some(index) => {
                let entry = &mut self.slab[index as usize];
                entry.packed = packed;
                entry.hash = hash;
                entry.position = position;
                entry.weight = weight;
                entry.live = true;
                index
            }
            None => {
                self.slab.push(Entry {
                    packed,
                    hash,
                    position,
                    weight,
                    generation: 0,
                    live: true,
                });
                (self.slab.len() - 1) as u32
            }
        };
        let generation = self.slab[index as usize].generation;
        let slab = &self.slab;
        self.table
            .insert_unique(hash, index, |&i| slab[i as usize].hash);
        self.phases[phase].push((index, generation));
        self.live += 1;
    }

    /// Remove the entry at `index`; its weight.
    fn remove(&mut self, index: u32) -> ModularWeight {
        let hash = self.slab[index as usize].hash;
        if let Ok(found) = self.table.find_entry(hash, |&i| i == index) {
            found.remove();
        }
        let entry = &mut self.slab[index as usize];
        entry.live = false;
        entry.generation = entry.generation.wrapping_add(1);
        self.free_slab.push(index);
        self.live -= 1;
        entry.weight
    }

    fn alloc_slot(&mut self, shard: usize) -> Result<u32, ReductionError> {
        let local = match self.free_slots.pop() {
            Some(local) => local,
            None => {
                let local = self.next_local;
                self.next_local += 1;
                local
            }
        };
        u32::try_from(local as usize * SHARDS + shard)
            .map_err(|_| invariant("too many layered frontier slots"))
    }

    fn free_slot(&mut self, slot: u32) {
        self.free_slots.push(slot / SHARDS as u32);
    }

    fn lock(shard: &Mutex<Self>) -> std::sync::MutexGuard<'_, Self> {
        shard
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// One term child of a window step, prepared on the worker.
struct LChild<K: FrontierKey, P> {
    key: K,
    packed: K::Packed,
    hash: u64,
    position: Option<OrderedPosition<P>>,
    id: u32,
    zero: bool,
    probe: Probe,
    /// Same phase, at or above the window threshold: a window key.
    window: bool,
}

/// A window step, prepared on the worker: children, terminals (shared-table
/// id and zero test) and conditions.
struct LStep<K: FrontierKey, P> {
    children: Vec<LChild<K, P>>,
    terminals: Vec<LTerminal>,
    conditions: Vec<Coefficient>,
}

type Node<K, P, E, A> = ClosureStep<K, P, LStep<K, P>, E, A>;

/// Event of one child in the shard merge.
const EV_NONE: u64 = 0;
const EV_NEW: u64 = 1;
const EV_ADD: u64 = 2;
const EV_CANCEL: u64 = 3;

#[inline]
fn event(kind: u64, slot: u32) -> u64 {
    (kind << 62) | u64::from(slot)
}

/// One expanded key of a layer.
struct Parent<P> {
    node: usize,
    weight: ModularWeight,
    priority: P,
}

/// The inspection view of the layered frontier (built lazily).
#[allow(clippy::type_complexity)]
struct View<'a, K: OrderedKernel> {
    shards: &'a [Mutex<Shard<<K::Key as FrontierKey>::Packed, K::Priority>>],
    terminals: &'a BTreeMap<IntegralKey, ModularWeight>,
    nonzero_conditions: &'a [Coefficient],
    unpacked: std::cell::OnceCell<Vec<(Scheduled<K::Key, K::Priority>, ModularWeight)>>,
}

impl<K: OrderedKernel> OrderedStateView for View<'_, K> {
    fn frontier(&self) -> Box<dyn Iterator<Item = OrderedFrontierEntry<'_>> + '_> {
        let unpacked = self.unpacked.get_or_init(|| {
            let mut all = Vec::new();
            for shard in self.shards {
                let shard = Shard::lock(shard);
                for entry in shard.slab.iter().filter(|e| e.live) {
                    all.push((
                        Scheduled {
                            phase: Reverse(entry.position.phase),
                            priority: entry.position.priority.clone(),
                            key: K::Key::unpack(&entry.packed),
                        },
                        entry.weight,
                    ));
                }
            }
            // Pop order: the greatest first.
            all.sort_by(|a, b| b.0.cmp(&a.0));
            all
        });
        Box::new(unpacked.iter().filter_map(|(scheduled, weight)| {
            frontier_entry::<K, ModularRecorder>(scheduled, weight)
        }))
    }

    fn terminals(
        &self,
    ) -> Box<dyn ExactSizeIterator<Item = (&IntegralKey, Option<&Coefficient>)> + '_> {
        Box::new(self.terminals.iter().map(|(key, _)| (key, None)))
    }

    fn nonzero_conditions(&self) -> &[Coefficient] {
        self.nonzero_conditions
    }
}

/// Whether a request with these settings runs layered.
pub(super) fn eligible<K: ConcurrentOrderedKernel>(
    kernel: &K,
    arithmetic: &OrderedWeightArithmetic,
    tail: &TailOptions,
) -> bool {
    tail.layered
        && !tail.enabled
        && !kernel.limits().diagnostics_enabled
        && matches!(
            arithmetic,
            OrderedWeightArithmetic::ModularSampled(..)
                | OrderedWeightArithmetic::ModularPolynomial(..)
        )
        && Lifter::of(*arithmetic).is_some_and(|(common, _)| !common.cross_check_exact)
        && kernel.detached_stepper().is_some()
}

/// Run one recorded request layered (see the module docs).
#[allow(clippy::too_many_arguments)]
pub(super) fn run<K>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    threads: usize,
    tail: TailOptions,
) -> Result<OrderedOutput, K::Error>
where
    K: ConcurrentOrderedKernel,
{
    let workers = Workers::new(threads)?;
    let (common, lifter) = Lifter::of(arithmetic).expect("modular arithmetic");
    let mut recorder = ModularRecorder::try_new(kernel.context(), kernel.limits(), common, lifter)?;
    recorder.set_modular_round(tail.modular_round);
    recorder.set_par_replay(tail.par_replay);
    recorder.set_prestage(true);
    let table = recorder
        .shared_intern()
        .ok_or_else(|| invariant("the layered mode needs the shared coefficient table"))?;
    let composer = kernel.terminal_composer();
    let stepper = kernel
        .detached_stepper()
        .ok_or_else(|| invariant("the layered mode needs a detached stepper"))?;
    // fwd-opt light steps on the workers (adapters that support them).
    workers.broadcast(|| stepper.set_light(true));
    let mut work = ReductionStatistics::default();
    let output = forward(
        kernel,
        &stepper,
        &workers,
        &mut recorder,
        &table,
        request,
        initial,
        nonzero_conditions,
        &mut work,
    );
    kernel.record_work(work);
    let output = output?;
    modular_output(recorder, output, composer.as_deref()).map_err(Into::into)
}

/// The forward pass: layered windows per phase.
#[allow(clippy::too_many_arguments)]
fn forward<K>(
    kernel: &mut K,
    stepper: &K::Detached,
    workers: &Workers,
    weights: &mut ModularRecorder,
    table: &std::sync::Arc<SharedIntern>,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    initial_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
) -> Result<RawOutput<ModularWeight>, K::Error>
where
    K: ConcurrentOrderedKernel,
{
    let limits = kernel.limits();
    let total_phases = kernel.phase_count();
    let field: ProbeField = weights.probe_field();
    let mut guards = GuardIndex::new();
    let mut nonzero_conditions = Vec::new();
    for condition in &initial_conditions {
        guards.intern(&mut nonzero_conditions, condition, |_| Ok(()))?;
    }
    let shards: Vec<Mutex<Shard<<K::Key as FrontierKey>::Packed, K::Priority>>> = (0..SHARDS)
        .map(|_| Mutex::new(Shard::new(total_phases)))
        .collect();
    let mut progress = CandidateOrderedReductionProgress {
        total_phases,
        input_keys: initial.len(),
        ..Default::default()
    };
    // Admission (sequential, in key order).
    for (key, coefficient) in initial {
        if coefficient.is_zero() {
            continue;
        }
        let position = kernel.position(&key)?;
        if position.phase >= total_phases {
            return Err(invariant("ordered entry phase is outside the frozen phase plan").into());
        }
        request.retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
        let packed = key.pack();
        let hash = FrontierHash::Fx.hash_one(&packed);
        let s = shard_of(hash);
        let mut shard = Shard::lock(&shards[s]);
        let slot = shard.alloc_slot(s)?;
        let (id, probe) =
            table.id_probe(&coefficient, super::modular::coefficient_hash(&coefficient));
        weights.layered_append(1)[0] = Op::layered(OpTag::Init, slot, id);
        shard.insert(packed, hash, position, ModularWeight::layered(slot, probe));
    }
    let live = |shards: &[Mutex<Shard<_, _>>]| -> usize {
        shards.iter().map(|s| Shard::lock(s).live).sum()
    };
    progress.frontier_keys = live(&shards);
    progress.peak_frontier_keys = progress.frontier_keys;
    progress.frontier_coefficient_monomials = progress.frontier_keys;
    progress.peak_frontier_coefficient_monomials = progress.frontier_keys;
    kernel.observe(&progress, None)?;
    let terminals = Mutex::new(Terminals {
        output: BTreeMap::new(),
        next_slot: 0,
    });
    let mut terminal_events: Vec<AtomicU64> = Vec::new();
    let mut lstats = LayerStats::default();
    let window = window_keys();
    let request_id = REQUESTS.fetch_add(1, AtomicOrdering::Relaxed);
    let mut par_clock = super::parstats::ForwardClock::new();
    let mut check: Option<std::collections::HashSet<<K::Key as FrontierKey>::Packed>> =
        check_requested().then(Default::default);
    let mut events: Vec<AtomicU64> = Vec::new();
    for phase in 0..total_phases {
        let phase_started = par_clock.now();
        let (mut pops, mut source) = (0u64, 0.0f64);
        weights.phase_started(phase);
        inspect::<K>(
            kernel,
            OrderedPhaseBoundary::Started,
            phase,
            &progress,
            &shards,
            &terminals
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .output,
            &nonzero_conditions,
        )?;
        loop {
            let roots_started = std::time::Instant::now();
            let roots = select_roots::<K>(workers, &shards, phase, window);
            lstats.roots += roots_started.elapsed().as_secs_f64();
            let Some((threshold_key, threshold_position)) = roots.last().cloned() else {
                break;
            };
            let threshold = Scheduled {
                phase: Reverse(phase),
                priority: threshold_position.priority,
                key: threshold_key,
            };
            // 1. The window closure, on the workers.
            let closure_started = par_clock.now();
            let mut nodes = closure::<K>(workers, stepper, table, roots, &threshold, request_id);
            if let Some(started) = closure_started {
                source += started.elapsed().as_secs_f64();
            }
            // 2. Pop order (greatest first) and layers.
            let layering_started = std::time::Instant::now();
            // On the workers: the sort, the packed keys and, per node, its
            // window children (node indices); then the layers in one serial
            // pass over the edges in pop order.
            workers.install(|| {
                use rayon::prelude::*;
                nodes.par_sort_unstable_by(|a, b| {
                    (&b.position.priority, &b.key).cmp(&(&a.position.priority, &a.key))
                });
            });
            let keys: Vec<&K::Key> = nodes.iter().map(|node| &node.key).collect();
            let lists: Vec<&[LChild<K::Key, K::Priority>]> = nodes
                .iter()
                .map(|node| match &node.result {
                    Ok(step) => step.children.as_slice(),
                    Err(_) => &[],
                })
                .collect();
            let (packed, hashes, kids) = workers.install(|| {
                use rayon::prelude::*;
                let (packed, hashes): (Vec<_>, Vec<u64>) = keys
                    .par_iter()
                    .map(|key| {
                        let packed = key.pack();
                        let hash = FrontierHash::Fx.hash_one(&packed);
                        (packed, hash)
                    })
                    .unzip();
                let mut index: HashTable<u32> = HashTable::with_capacity(keys.len());
                for (i, &hash) in hashes.iter().enumerate() {
                    index.insert_unique(hash, i as u32, |&j| hashes[j as usize]);
                }
                let kids: Vec<Vec<u32>> = lists
                    .par_iter()
                    .map(|children| {
                        children
                            .iter()
                            .filter(|c| c.window)
                            .filter_map(|child| {
                                index
                                    .find(child.hash, |&j| packed[j as usize] == child.packed)
                                    .copied()
                            })
                            .collect()
                    })
                    .collect();
                (packed, hashes, kids)
            });
            drop((keys, lists));
            let mut layer = vec![0u32; nodes.len()];
            for (i, kids) in kids.iter().enumerate() {
                let next = layer[i] + 1;
                for &j in kids {
                    let j = j as usize;
                    if j <= i {
                        return Err(invariant(
                            "a window child is not below its parent in the pop order",
                        )
                        .into());
                    }
                    layer[j] = layer[j].max(next);
                }
            }
            drop(kids);
            let mut order: Vec<u32> = (0..nodes.len() as u32).collect();
            order.sort_by_key(|&i| (layer[i as usize], i));
            lstats.layering += layering_started.elapsed().as_secs_f64();
            // 3. Layers.
            let mut expanded: Vec<usize> = Vec::new();
            let mut start = 0;
            while start < order.len() {
                let current = layer[order[start] as usize];
                let end = start
                    + order[start..]
                        .iter()
                        .take_while(|&&i| layer[i as usize] == current)
                        .count();
                let members = &order[start..end];
                start = end;
                let layer_started = std::time::Instant::now();
                // The expanded keys: those still pending (others cancelled
                // or never received a nonzero weight).
                let mut parents: Vec<Parent<K::Priority>> = Vec::with_capacity(members.len());
                for &i in members {
                    let i = i as usize;
                    let s = shard_of(hashes[i]);
                    let mut shard = Shard::lock(&shards[s]);
                    if let Some(at) = shard.find(hashes[i], &packed[i]) {
                        let weight = shard.remove(at);
                        parents.push(Parent {
                            node: i,
                            weight,
                            priority: nodes[i].position.priority.clone(),
                        });
                    }
                }
                if parents.is_empty() {
                    continue;
                }
                // The first failed step stops the layer there.
                let failed = parents.iter().position(|p| nodes[p.node].result.is_err());
                let merged = failed.unwrap_or(parents.len());
                let steps: Vec<&LStep<K::Key, K::Priority>> = parents[..merged]
                    .iter()
                    .map(|parent| match &nodes[parent.node].result {
                        Ok(step) => step,
                        Err(_) => unreachable!("failed steps are cut off above"),
                    })
                    .collect();
                // Offsets of the children and terminals of each parent.
                let mut child_offsets = Vec::with_capacity(merged + 1);
                let mut terminal_offsets = Vec::with_capacity(merged + 1);
                let (mut children_total, mut terminals_total) = (0usize, 0usize);
                for step in &steps {
                    child_offsets.push(children_total);
                    terminal_offsets.push(terminals_total);
                    children_total += step.children.len();
                    terminals_total += step.terminals.len();
                }
                child_offsets.push(children_total);
                terminal_offsets.push(terminals_total);
                for (buffer, total) in [
                    (&mut events, children_total),
                    (&mut terminal_events, terminals_total),
                ] {
                    if buffer.len() < total {
                        buffer.resize_with(total, || AtomicU64::new(EV_NONE));
                    }
                    for e in &buffer[..total] {
                        e.store(EV_NONE, AtomicOrdering::Relaxed);
                    }
                }
                let merge_started = std::time::Instant::now();
                let mut shard_error = merge_layer::<K>(
                    workers,
                    stepper,
                    &shards,
                    &field,
                    &steps,
                    &parents[..merged],
                    &child_offsets,
                    &events[..children_total],
                    &terminal_offsets,
                    &terminal_events[..terminals_total],
                    &terminals,
                    phase,
                    total_phases,
                );
                lstats.merge += merge_started.elapsed().as_secs_f64();
                // The ops of each parent: Pop, its children's events, its
                // terminals' events (raw shared-table ids).
                let ops_started = std::time::Instant::now();
                let stop = shard_error.as_ref().map(|(p, _)| *p).unwrap_or(merged);
                let mut op_offsets = Vec::with_capacity(stop + 1);
                let mut ops_total = 0usize;
                let (mut new, mut add, mut cancel, mut term_add, mut term_cancel, mut emitted) =
                    (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
                for p in 0..stop {
                    op_offsets.push(ops_total);
                    ops_total += 1;
                    for e in &events[child_offsets[p]..child_offsets[p + 1]] {
                        match e.load(AtomicOrdering::Relaxed) >> 62 {
                            EV_NONE => {}
                            EV_NEW => (new, ops_total) = (new + 1, ops_total + 1),
                            EV_ADD => (add, ops_total) = (add + 1, ops_total + 1),
                            _ => (cancel, ops_total) = (cancel + 1, ops_total + 2),
                        }
                    }
                    for e in &terminal_events[terminal_offsets[p]..terminal_offsets[p + 1]] {
                        match e.load(AtomicOrdering::Relaxed) >> 62 {
                            EV_NONE => {}
                            EV_NEW => (emitted, ops_total) = (emitted + 1, ops_total + 1),
                            EV_ADD => (term_add, ops_total) = (term_add + 1, ops_total + 1),
                            _ => (term_cancel, ops_total) = (term_cancel + 1, ops_total + 2),
                        }
                    }
                }
                op_offsets.push(ops_total);
                write_ops::<K>(
                    workers,
                    weights.layered_append(ops_total),
                    &steps[..stop],
                    &parents[..stop],
                    &op_offsets,
                    &child_offsets,
                    &events,
                    &terminal_offsets,
                    &terminal_events,
                );
                lstats.ops += ops_started.elapsed().as_secs_f64();
                drop(steps);
                // Accounting, in parent order (the scheduler's effects).
                let serial_started = std::time::Instant::now();
                let terminal_new = emitted;
                let emitted = new + add + cancel + terminal_new + term_add + term_cancel;
                for (p, parent) in parents.iter().enumerate() {
                    request.release_pending_frame()?;
                    pops += 1;
                    if let Some(set) = &mut check
                        && !set.insert(packed[parent.node].clone())
                    {
                        return Err(invariant("a layered key was expanded twice").into());
                    }
                    progress.frontier_keys = progress.frontier_keys.saturating_sub(1);
                    let failed = nodes[parent.node].result.is_err();
                    let stopped = shard_error.as_ref().is_some_and(|(at, _)| *at == p);
                    if failed || stopped {
                        // The scheduler's effects up to the failure, then
                        // the failure (the request ends).
                        expanded.push(parent.node);
                        account_window::<K>(
                            kernel,
                            &mut nodes,
                            &mut expanded,
                            &mut guards,
                            &mut nonzero_conditions,
                            &mut progress,
                            request,
                            work,
                        )?;
                        if let Some((_, error)) = shard_error.take() {
                            return Err(error);
                        }
                        return Err(invariant("a failed layered step was absorbed").into());
                    }
                    expanded.push(parent.node);
                }
                // Layer totals (the same sums as one event at a time).
                for _ in 0..new {
                    request
                        .retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
                }
                for _ in 0..cancel {
                    request.release_pending_frame()?;
                }
                let coalesced = add + cancel + term_add + term_cancel;
                work.record_coalescing_additions(coalesced);
                request.record_coalescing_additions(
                    coalesced,
                    limits.diagnostic_limit(limits.max_coalescing_additions),
                )?;
                progress.emitted_terms = progress.emitted_terms.saturating_add(emitted);
                progress.merged_terms = progress.merged_terms.saturating_add(add);
                progress.cancelled_terms = progress.cancelled_terms.saturating_add(cancel);
                progress.frontier_keys = progress.frontier_keys + new - cancel;
                progress.peak_frontier_keys =
                    progress.peak_frontier_keys.max(progress.frontier_keys);
                progress.frontier_coefficient_monomials = progress.frontier_keys;
                progress.peak_frontier_coefficient_monomials = progress
                    .peak_frontier_coefficient_monomials
                    .max(progress.frontier_keys);
                {
                    let terminals = terminals
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    progress.terminal_keys = terminals.output.len();
                    progress.terminal_merges = progress.terminal_merges.saturating_add(term_add);
                    progress.terminal_cancellations =
                        progress.terminal_cancellations.saturating_add(term_cancel);
                    progress.terminal_coefficient_monomials = terminals.output.len();
                    progress.peak_terminal_coefficient_monomials = progress
                        .peak_terminal_coefficient_monomials
                        .max(terminals.output.len());
                }
                // The expanded weights are spent: their slots return to
                // their shards (after every op of the layer).
                for parent in &parents {
                    let s = shard_of(hashes[parent.node]);
                    Shard::lock(&shards[s]).free_slot(parent.weight.slot());
                }
                lstats.serial += serial_started.elapsed().as_secs_f64();
                lstats.layers += 1;
                lstats.layer_wall += layer_started.elapsed().as_secs_f64();
            }
            // The scheduler's per-key effects (observations, step accounts,
            // conditions) in the sequential pop order of the window.
            let accounted = std::time::Instant::now();
            account_window::<K>(
                kernel,
                &mut nodes,
                &mut expanded,
                &mut guards,
                &mut nonzero_conditions,
                &mut progress,
                request,
                work,
            )?;
            lstats.serial += accounted.elapsed().as_secs_f64();
        }
        if let Some(started) = phase_started {
            let wall = started.elapsed().as_secs_f64();
            par_clock.phase(phase, pops, wall, source, wall - source);
        }
        // The phase has drained: nothing pending in it or before it.
        for shard in &shards {
            let shard = Shard::lock(shard);
            if shard.phases[..=phase]
                .iter()
                .flatten()
                .any(|&(index, generation)| {
                    let entry = &shard.slab[index as usize];
                    entry.live && entry.generation == generation
                })
            {
                return Err(invariant("ordered phase was regenerated after completion").into());
            }
        }
        progress.completed_phases = phase + 1;
        kernel.observe(&progress, None)?;
        inspect::<K>(
            kernel,
            OrderedPhaseBoundary::Finished,
            phase,
            &progress,
            &shards,
            &terminals
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .output,
            &nonzero_conditions,
        )?;
    }
    par_clock.report(workers.threads());
    let mut slots = 0u32;
    for (s, shard) in shards.iter().enumerate() {
        let shard = Shard::lock(shard);
        if shard.live != 0 {
            return Err(invariant("ordered frontier remains after the last phase").into());
        }
        if shard.next_local > 0 {
            slots = slots.max((shard.next_local - 1) * SHARDS as u32 + s as u32 + 1);
        }
    }
    weights.layered_slots(slots);
    let Terminals { output, next_slot } = terminals
        .into_inner()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    weights.layered_terminal_slots(next_slot);
    // Coefficient references in sequence order (first occurrence).
    let mapped = std::time::Instant::now();
    weights.layered_map_refs(0)?;
    lstats.map = mapped.elapsed().as_secs_f64();
    lstats.report(workers.threads());
    progress.terminal_keys = output.len();
    Ok(RawOutput {
        terms: output,
        nonzero_conditions,
        work: *work,
        progress,
    })
}

impl<K: FrontierKey, P> LStep<K, P> {
    fn empty() -> Self {
        Self {
            children: Vec::new(),
            terminals: Vec::new(),
            conditions: Vec::new(),
        }
    }
}

/// The scheduler's per-key effects of the expanded keys of a window, in the
/// sequential pop order (node order): observation before and after each
/// step, its account, its conditions (first-appearance order), the processed
/// counter. A failed step's account is applied, then its error returned.
#[allow(clippy::too_many_arguments)]
fn account_window<K>(
    kernel: &mut K,
    nodes: &mut [Node<K::Key, K::Priority, K::Error, K::Account>],
    expanded: &mut Vec<usize>,
    guards: &mut GuardIndex,
    nonzero_conditions: &mut Vec<Coefficient>,
    progress: &mut CandidateOrderedReductionProgress,
    request: &mut ReductionRequest,
    work: &mut ReductionStatistics,
) -> Result<(), K::Error>
where
    K: ConcurrentOrderedKernel,
{
    expanded.sort_unstable();
    for &i in expanded.iter() {
        let node = &mut nodes[i];
        kernel.observe(progress, Some(&node.key))?;
        let account = node
            .account
            .take()
            .ok_or_else(|| invariant("a layered step was absorbed twice"))?;
        kernel.absorb(&node.key, account, request, work)?;
        let step = match &mut node.result {
            Ok(step) => step,
            Err(_) => {
                let failed = std::mem::replace(&mut node.result, Ok(LStep::empty()));
                return Err(failed.err().expect("a failed step"));
            }
        };
        for condition in std::mem::take(&mut step.conditions) {
            guards.intern(nonzero_conditions, &condition, |_| Ok(()))?;
        }
        progress.processed_keys = progress.processed_keys.saturating_add(1);
        kernel.observe(progress, Some(&node.key))?;
    }
    expanded.clear();
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn inspect<K>(
    kernel: &mut K,
    boundary: OrderedPhaseBoundary,
    phase: usize,
    progress: &CandidateOrderedReductionProgress,
    shards: &[Mutex<Shard<<K::Key as FrontierKey>::Packed, K::Priority>>],
    output: &BTreeMap<IntegralKey, ModularWeight>,
    nonzero_conditions: &[Coefficient],
) -> Result<(), K::Error>
where
    K: ConcurrentOrderedKernel,
{
    let view: View<'_, K> = View {
        shards,
        terminals: output,
        nonzero_conditions,
        unpacked: std::cell::OnceCell::new(),
    };
    kernel.inspect_phase(OrderedPhaseEvent::new(boundary, phase, *progress, &view))
}

/// The greatest `window` pending keys of `phase` (greatest first), with
/// their positions.
#[allow(clippy::type_complexity)]
fn select_roots<K>(
    workers: &Workers,
    shards: &[Mutex<Shard<<K::Key as FrontierKey>::Packed, K::Priority>>],
    phase: usize,
    window: usize,
) -> Vec<(K::Key, OrderedPosition<K::Priority>)>
where
    K: ConcurrentOrderedKernel,
{
    let cmp = |a: &Entry<<K::Key as FrontierKey>::Packed, K::Priority>,
               b: &Entry<<K::Key as FrontierKey>::Packed, K::Priority>| {
        b.position
            .priority
            .cmp(&a.position.priority)
            .then_with(|| K::Key::cmp_packed(&b.packed, &a.packed))
    };
    // Per shard (on the workers): its greatest `window` keys, sorted.
    let tops: Vec<Mutex<Vec<u32>>> = (0..SHARDS).map(|_| Mutex::new(Vec::new())).collect();
    workers.for_each_index(SHARDS, |s| {
        let mut shard = Shard::lock(&shards[s]);
        let Shard { slab, phases, .. } = &mut *shard;
        let list = &mut phases[phase];
        list.retain(|&(index, generation)| {
            let entry = &slab[index as usize];
            entry.live && entry.generation == generation
        });
        let mut top: Vec<u32> = list.iter().map(|&(index, _)| index).collect();
        let by = |a: &u32, b: &u32| cmp(&slab[*a as usize], &slab[*b as usize]);
        if top.len() > window {
            top.select_nth_unstable_by(window - 1, by);
            top.truncate(window);
        }
        top.sort_unstable_by(by);
        *tops[s]
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = top;
    });
    let guards: Vec<_> = shards.iter().map(Shard::lock).collect();
    let tops: Vec<Vec<u32>> = tops
        .into_iter()
        .map(|t| {
            t.into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
        .collect();
    // k-way merge of the sorted shard tops with a binary heap of shards
    // (the shard whose head comes first in pop order at the root).
    let mut heads = vec![0usize; SHARDS];
    let head = |s: usize, heads: &[usize]| &guards[s].slab[tops[s][heads[s]] as usize];
    let before = |a: usize, b: usize, heads: &[usize]| {
        cmp(head(a, heads), head(b, heads)) == Ordering::Less
            || (cmp(head(a, heads), head(b, heads)) == Ordering::Equal && a < b)
    };
    let mut heap: Vec<usize> = (0..SHARDS).filter(|&s| !tops[s].is_empty()).collect();
    let sift_down = |heap: &mut Vec<usize>, mut at: usize, heads: &[usize]| loop {
        let (l, r) = (2 * at + 1, 2 * at + 2);
        let mut first = at;
        if l < heap.len() && before(heap[l], heap[first], heads) {
            first = l;
        }
        if r < heap.len() && before(heap[r], heap[first], heads) {
            first = r;
        }
        if first == at {
            break;
        }
        heap.swap(at, first);
        at = first;
    };
    for at in (0..heap.len() / 2).rev() {
        sift_down(&mut heap, at, &heads);
    }
    let mut roots = Vec::with_capacity(window);
    while roots.len() < window && !heap.is_empty() {
        let s = heap[0];
        let entry = head(s, &heads);
        roots.push((K::Key::unpack(&entry.packed), entry.position.clone()));
        heads[s] += 1;
        if heads[s] == tops[s].len() {
            let last = heap.len() - 1;
            heap.swap(0, last);
            heap.pop();
        }
        sift_down(&mut heap, 0, &heads);
    }
    roots
}

/// The steps of a window closure, prepared for the layered merge.
#[allow(clippy::type_complexity)]
fn closure<K>(
    workers: &Workers,
    stepper: &K::Detached,
    table: &SharedIntern,
    roots: Vec<(K::Key, OrderedPosition<K::Priority>)>,
    threshold: &Scheduled<K::Key, K::Priority>,
    request_id: u64,
) -> Vec<Node<K::Key, K::Priority, K::Error, K::Account>>
where
    K: ConcurrentOrderedKernel,
{
    layered_closure(workers, stepper, roots, |parent, _, step, found| {
        let OrderedStep {
            terms,
            terminals,
            nonzero_conditions,
        } = step;
        // Light children (singly met memo children, by handle) merge with
        // the materialised terms in key order: the children are those of
        // the full step, in its order.
        let mut light = stepper.take_light().into_iter().peekable();
        let mut materialised = terms.into_iter().peekable();
        let mut merged: Vec<(K::Key, Result<Coefficient, u64>)> =
            Vec::with_capacity(materialised.len() + light.len());
        loop {
            let take_light = match (materialised.peek(), light.peek()) {
                (None, None) => break,
                (Some(_), None) => false,
                (None, Some(_)) => true,
                (Some((a, _)), Some((b, _))) => b < a,
            };
            if take_light {
                let (key, handle) = light.next().expect("peeked");
                merged.push((key, Err(handle)));
            } else {
                let (key, coefficient) = materialised.next().expect("peeked");
                merged.push((key, Ok(coefficient)));
            }
        }
        let mut children = Vec::with_capacity(merged.len());
        for (child, source) in merged {
            let (id, probe, zero) = match source {
                Ok(coefficient) => {
                    let zero = coefficient.is_zero();
                    let (id, probe) = table
                        .id_probe(&coefficient, super::modular::coefficient_hash(&coefficient));
                    (id, probe, zero)
                }
                Err(handle) => HANDLES.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    if cache.0 != request_id {
                        cache.0 = request_id;
                        cache.1.clear();
                    }
                    if let Some(&found) = cache.1.get(&handle) {
                        return found;
                    }
                    let coefficient = stepper
                        .light_value(handle)
                        .expect("a light handle names a shared memo coefficient");
                    let zero = coefficient.is_zero();
                    let (id, probe) = table
                        .id_probe(&coefficient, super::modular::coefficient_hash(&coefficient));
                    cache.1.insert(handle, (id, probe, zero));
                    (id, probe, zero)
                }),
            };
            if zero {
                // A zero weight is no child (the merged step has no such term).
                continue;
            }
            // A child of a later phase needs no position here: the shard
            // computes it if the child is new.
            let position = if stepper.later_phase(parent, &child) {
                None
            } else {
                stepper.position(&child).ok()
            };
            let window = position
                .as_ref()
                .is_some_and(|position| at_or_above(position, &child, threshold));
            if window && let Some(position) = &position {
                found.push((child.clone(), position.clone()));
            }
            let packed = child.pack();
            let hash = FrontierHash::Fx.hash_one(&packed);
            children.push(LChild {
                key: child,
                packed,
                hash,
                position,
                id,
                zero,
                probe,
                window,
            });
        }
        let terminals = terminals
            .into_iter()
            .map(|(terminal, coefficient)| {
                let zero = coefficient.is_zero();
                let (id, probe) =
                    table.id_probe(&coefficient, super::modular::coefficient_hash(&coefficient));
                LTerminal {
                    key: terminal,
                    id,
                    zero,
                    probe,
                }
            })
            .collect();
        LStep {
            children,
            terminals,
            conditions: nonzero_conditions,
        }
    })
}

/// The terminal accumulators (merged by one task per layer, in parent
/// order).
struct Terminals {
    output: BTreeMap<IntegralKey, ModularWeight>,
    next_slot: u32,
}

/// One terminal contribution of a window step.
struct LTerminal {
    key: IntegralKey,
    id: u32,
    zero: bool,
    probe: Probe,
}

/// Wall-time split of the layered forward pass (`RUSTRED_PAR_STATS`).
#[derive(Default)]
struct LayerStats {
    layers: u64,
    layer_wall: f64,
    merge: f64,
    ops: f64,
    serial: f64,
    roots: f64,
    layering: f64,
    map: f64,
}

impl LayerStats {
    fn report(&self, threads: usize) {
        super::parstats::write(&format!(
            "{{\"stage\":\"layered\",\"threads\":{threads},\"layers\":{},\"layer_wall\":{:.6},\
             \"merge\":{:.6},\"ops\":{:.6},\"serial\":{:.6},\"roots\":{:.6},\"layering\":{:.6},\
             \"map\":{:.6}}}",
            self.layers,
            self.layer_wall,
            self.merge,
            self.ops,
            self.serial,
            self.roots,
            self.layering,
            self.map
        ));
    }
}

/// The shard-wise merge of one layer: every nonzero child contribution of
/// `parents` lands in its destination shard, in (parent, child) order, and
/// every terminal contribution in the terminal accumulators, in (parent,
/// terminal) order; the outcome of each is written to `events` /
/// `terminal_events`. Returns the first error in parent order, if any (the
/// merge of that shard stops there).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn merge_layer<K>(
    workers: &Workers,
    stepper: &K::Detached,
    shards: &[Mutex<Shard<<K::Key as FrontierKey>::Packed, K::Priority>>],
    field: &ProbeField,
    steps: &[&LStep<K::Key, K::Priority>],
    parents: &[Parent<K::Priority>],
    child_offsets: &[usize],
    events: &[AtomicU64],
    terminal_offsets: &[usize],
    terminal_events: &[AtomicU64],
    terminals: &Mutex<Terminals>,
    phase: usize,
    total_phases: usize,
) -> Option<(usize, K::Error)>
where
    K: ConcurrentOrderedKernel,
{
    let total = events.len() + terminal_events.len();
    if total == 0 {
        return None;
    }
    // Small layers merge on the calling thread (same order, same outcome).
    let parallel = workers.threads() > 1 && total >= 2048;
    let chunks = if parallel {
        (workers.threads() * 4).min(parents.len()).max(1)
    } else {
        1
    };
    let per_chunk = parents.len().div_ceil(chunks);
    // Pass 1: per parent chunk, per destination shard: (parent, child).
    let bucket = |c: usize| -> Vec<Vec<(u32, u32)>> {
        let mut out: Vec<Vec<(u32, u32)>> = (0..SHARDS).map(|_| Vec::new()).collect();
        let end = ((c + 1) * per_chunk).min(parents.len());
        for p in c * per_chunk..end {
            for (i, child) in steps[p].children.iter().enumerate() {
                if !child.zero {
                    out[shard_of(child.hash)].push((p as u32, i as u32));
                }
            }
        }
        out
    };
    let buckets: Vec<Vec<Vec<(u32, u32)>>> = if parallel {
        let slots: Vec<Mutex<Vec<Vec<(u32, u32)>>>> =
            (0..chunks).map(|_| Mutex::new(Vec::new())).collect();
        workers.for_each_index(chunks, |c| {
            *slots[c]
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = bucket(c);
        });
        slots
            .into_iter()
            .map(|s| {
                s.into_inner()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            })
            .collect()
    } else {
        (0..chunks).map(bucket).collect()
    };
    // Pass 2: per shard, the merges in (parent, child) order; one more task
    // merges the terminals.
    let errors: Vec<Mutex<Option<(usize, usize, K::Error)>>> =
        (0..=SHARDS).map(|_| Mutex::new(None)).collect();
    let merge_shard = |s: usize| {
        if s == SHARDS {
            let mut terminals = terminals
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Terminals { output, next_slot } = &mut *terminals;
            for (p, step) in steps.iter().enumerate() {
                for (k, terminal) in step.terminals.iter().enumerate() {
                    if terminal.zero {
                        continue;
                    }
                    let contribution = field.scale(parents[p].weight.probe(), &terminal.probe);
                    let value = match output.get_mut(&terminal.key) {
                        Some(existing) => {
                            let slot = existing.slot();
                            if field.add_is_zero(existing.probe_mut(), &contribution) {
                                output.remove(&terminal.key);
                                event(EV_CANCEL, slot)
                            } else {
                                event(EV_ADD, slot)
                            }
                        }
                        None => {
                            let slot = *next_slot;
                            *next_slot += 1;
                            output.insert(
                                terminal.key.clone(),
                                ModularWeight::layered(slot, contribution),
                            );
                            event(EV_NEW, slot)
                        }
                    };
                    terminal_events[terminal_offsets[p] + k].store(value, AtomicOrdering::Relaxed);
                }
            }
            return;
        }
        let mut shard = Shard::lock(&shards[s]);
        for chunk in &buckets {
            for &(p, i) in &chunk[s] {
                let (p, i) = (p as usize, i as usize);
                let parent = &parents[p];
                let child = &steps[p].children[i];
                let contribution = field.scale(parent.weight.probe(), &child.probe);
                let outcome: Result<u64, K::Error> = (|| match shard.find(child.hash, &child.packed)
                {
                    Some(at) => {
                        let entry = &mut shard.slab[at as usize];
                        check_descent_parts(
                            entry.position.phase,
                            &entry.position.priority,
                            phase,
                            &parent.priority,
                            total_phases,
                        )?;
                        let slot = entry.weight.slot();
                        if field.add_is_zero(entry.weight.probe_mut(), &contribution) {
                            shard.remove(at);
                            shard.free_slot(slot);
                            Ok(event(EV_CANCEL, slot))
                        } else {
                            Ok(event(EV_ADD, slot))
                        }
                    }
                    None => {
                        let position = match &child.position {
                            Some(position) => position.clone(),
                            None => stepper.position(&child.key)?,
                        };
                        check_descent(&position, phase, &parent.priority, total_phases)?;
                        let slot = shard.alloc_slot(s)?;
                        shard.insert(
                            child.packed.clone(),
                            child.hash,
                            position,
                            ModularWeight::layered(slot, contribution),
                        );
                        Ok(event(EV_NEW, slot))
                    }
                })();
                match outcome {
                    Ok(value) => events[child_offsets[p] + i].store(value, AtomicOrdering::Relaxed),
                    Err(error) => {
                        *errors[s]
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner) =
                            Some((p, i, error));
                        return;
                    }
                }
            }
        }
    };
    if parallel {
        workers.for_each_index(SHARDS + 1, merge_shard);
    } else {
        (0..=SHARDS).for_each(merge_shard);
    }
    errors
        .into_iter()
        .filter_map(|e| {
            e.into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
        })
        .min_by_key(|(p, i, _)| (*p, *i))
        .map(|(p, _, error)| (p, error))
}

/// The ops of `parents` at `op_offsets` (parallel by parent chunks): `Pop`,
/// then each child's event, then each terminal's event.
#[allow(clippy::too_many_arguments)]
fn write_ops<K>(
    workers: &Workers,
    out: &mut [Op],
    steps: &[&LStep<K::Key, K::Priority>],
    parents: &[Parent<K::Priority>],
    op_offsets: &[usize],
    child_offsets: &[usize],
    events: &[AtomicU64],
    terminal_offsets: &[usize],
    terminal_events: &[AtomicU64],
) where
    K: ConcurrentOrderedKernel,
{
    let fill = |p: usize, ops: &mut [Op]| {
        let mut at = 0;
        ops[at] = Op::bare(OpTag::Pop, parents[p].weight.slot());
        at += 1;
        for (i, child) in steps[p].children.iter().enumerate() {
            let value = events[child_offsets[p] + i].load(AtomicOrdering::Relaxed);
            let slot = (value & 0xFFFF_FFFF) as u32;
            match value >> 62 {
                EV_NONE => {}
                EV_NEW => {
                    ops[at] = Op::layered(OpTag::New, slot, child.id);
                    at += 1;
                }
                EV_ADD => {
                    ops[at] = Op::layered(OpTag::Add, slot, child.id);
                    at += 1;
                }
                _ => {
                    ops[at] = Op::layered(OpTag::Add, slot, child.id);
                    ops[at + 1] = Op::bare(OpTag::Cancel, slot);
                    at += 2;
                }
            }
        }
        for (k, terminal) in steps[p].terminals.iter().enumerate() {
            let value = terminal_events[terminal_offsets[p] + k].load(AtomicOrdering::Relaxed);
            let slot = (value & 0xFFFF_FFFF) as u32;
            match value >> 62 {
                EV_NONE => {}
                EV_NEW => {
                    ops[at] = Op::layered(OpTag::TermNew, slot, terminal.id);
                    at += 1;
                }
                EV_ADD => {
                    ops[at] = Op::layered(OpTag::TermAdd, slot, terminal.id);
                    at += 1;
                }
                _ => {
                    ops[at] = Op::layered(OpTag::TermAdd, slot, terminal.id);
                    ops[at + 1] = Op::bare(OpTag::TermCancel, slot);
                    at += 2;
                }
            }
        }
        debug_assert_eq!(at, ops.len());
    };
    let parents_len = parents.len();
    let total = *op_offsets.last().unwrap_or(&0);
    if workers.threads() == 1 || total < 8192 {
        for p in 0..parents_len {
            fill(p, &mut out[op_offsets[p]..op_offsets[p + 1]]);
        }
        return;
    }
    let chunks = (workers.threads() * 4).min(parents_len).max(1);
    let per_chunk = parents_len.div_ceil(chunks);
    let mut slices: Vec<Mutex<(usize, &mut [Op])>> = Vec::with_capacity(chunks);
    let mut rest = out;
    let mut consumed = 0;
    for c in 0..chunks {
        let (a, b) = (c * per_chunk, ((c + 1) * per_chunk).min(parents_len));
        if a >= b {
            break;
        }
        let (head, tail) = rest.split_at_mut(op_offsets[b] - consumed);
        consumed = op_offsets[b];
        slices.push(Mutex::new((a, head)));
        rest = tail;
    }
    workers.for_each_index(slices.len(), |c| {
        let mut guard = slices[c]
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (a, ref mut ops) = *guard;
        let b = ((c + 1) * per_chunk).min(parents_len);
        let base = op_offsets[a];
        for p in a..b {
            fill(p, &mut ops[op_offsets[p] - base..op_offsets[p + 1] - base]);
        }
    });
}
