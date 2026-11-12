//! rr-par: one replay pass on several threads (`OrderedTuning::par_replay`:
//! on for p requests since M5 batch 3; `RUSTRED_MODULAR_PAR_REPLAY=on|off`
//! overrides it).
//!
//! The trace is a sequence of steps: a `Pop` of a frontier slot followed by
//! the products of the popped (active) weight into other slots (`New`,
//! `Add`) and into terminal accumulators (`TermNew`, `TermAdd`), with the
//! zero checks of erased keys (`Cancel`, `TermCancel`). Renaming every slot
//! lifetime to its own value (a slot is reused once its key is popped or
//! erased) turns the trace into a dataflow graph: a step can run as soon as
//! every write into the value it pops has run. Writes into one value commute:
//! the partial-fraction weights of `fraction.rs` are a unique representation
//! of an element of `Z_p(d)` and a sum is coefficient-wise modulo `p`, so the
//! value a step pops, every zero check and every terminal value are the same
//! elements of `Z_p(d)` in every order (only the storage layout of a weight,
//! which never enters a value, may differ). The images of the terminals are
//! reduced and made monic from those values, hence identical to the
//! sequential pass's.
//!
//! Execution: every value counts its pending writes; a write that completes
//! a value starts its consumer (the popping step on the pool, or its zero
//! check). Each pool thread owns one weight arithmetic (scratch and caches)
//! and its own partial sums of the terminal accumulators, which are added
//! after the pass. A value is guarded by its own lock while a product is
//! added into it.
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use symbolica::utils::resources::{self, ResourceBudget};

use super::super::lift::Image;
use super::super::{OpTag, Trace, error};
use super::Representation;
use crate::reduction::ReductionError;

const NONE: u32 = u32::MAX;
const CANCELLED: u32 = u32::MAX - 1;

/// A weight arithmetic that can add one weight into another (the sum of the
/// per-thread terminal partials).
pub(super) trait Summable: Representation {
    fn add_weight(
        &mut self,
        destination: &mut Self::Weight,
        source: &Self::Weight,
    ) -> Result<(), ReductionError>;
}

/// The dataflow graph of a trace (values are renamed slot lifetimes).
struct Dataflow {
    /// Per step: the popped value and its op range (after the `Pop`).
    steps: Vec<(u32, u32, u32)>,
    /// Per op: the value or terminal slot it writes, else `NONE`.
    target: Vec<u32>,
    /// Writes per value.
    writes: Vec<u32>,
    /// Consumer per value: a step index, `CANCELLED` or `NONE`.
    consumer: Vec<u32>,
    /// Ops that create a value from a coefficient.
    inits: Vec<u32>,
    /// Terminal slots erased as zero.
    terminal_cancelled: Vec<bool>,
    cancel_checks: usize,
}

impl Dataflow {
    fn build(trace: &Trace) -> Result<Self, ReductionError> {
        let unsupported = || error("trace shape not supported by the parallel replay");
        let n = trace.ops.len();
        if n >= NONE as usize - 1 {
            return Err(unsupported());
        }
        let mut slot_value = vec![NONE; trace.slots];
        let mut flow = Dataflow {
            steps: Vec::new(),
            target: vec![NONE; n],
            writes: Vec::new(),
            consumer: Vec::new(),
            inits: Vec::new(),
            terminal_cancelled: vec![false; trace.terminal_slots],
            cancel_checks: 0,
        };
        let new_value = |flow: &mut Dataflow| -> Result<u32, ReductionError> {
            let value = u32::try_from(flow.writes.len()).map_err(|_| unsupported())?;
            if value >= CANCELLED {
                return Err(unsupported());
            }
            flow.writes.push(0);
            flow.consumer.push(NONE);
            Ok(value)
        };
        let mut open: Option<usize> = None;
        for (index, op) in trace.ops.iter().enumerate() {
            let slot = op.slot();
            match op.tag() {
                OpTag::Init => {
                    let value = new_value(&mut flow)?;
                    *slot_value.get_mut(slot).ok_or_else(unsupported)? = value;
                    flow.target[index] = value;
                    flow.writes[value as usize] += 1;
                    flow.inits.push(index as u32);
                }
                OpTag::Pop => {
                    if let Some(step) = open {
                        flow.steps[step].2 = index as u32;
                    }
                    let value =
                        std::mem::replace(slot_value.get_mut(slot).ok_or_else(unsupported)?, NONE);
                    if value == NONE || flow.consumer[value as usize] != NONE {
                        return Err(unsupported());
                    }
                    flow.consumer[value as usize] = flow.steps.len() as u32;
                    open = Some(flow.steps.len());
                    flow.steps.push((value, index as u32 + 1, n as u32));
                }
                OpTag::New => {
                    if open.is_none() {
                        return Err(unsupported());
                    }
                    let value = new_value(&mut flow)?;
                    *slot_value.get_mut(slot).ok_or_else(unsupported)? = value;
                    flow.target[index] = value;
                    flow.writes[value as usize] += 1;
                }
                OpTag::Add => {
                    let value = *slot_value.get(slot).ok_or_else(unsupported)?;
                    if open.is_none() || value == NONE {
                        return Err(unsupported());
                    }
                    flow.target[index] = value;
                    flow.writes[value as usize] += 1;
                }
                OpTag::Cancel => {
                    let value =
                        std::mem::replace(slot_value.get_mut(slot).ok_or_else(unsupported)?, NONE);
                    if value == NONE || flow.consumer[value as usize] != NONE {
                        return Err(unsupported());
                    }
                    flow.consumer[value as usize] = CANCELLED;
                    flow.cancel_checks += 1;
                }
                OpTag::TermNew | OpTag::TermAdd => {
                    if open.is_none() || slot >= trace.terminal_slots {
                        return Err(unsupported());
                    }
                    flow.target[index] = slot as u32;
                }
                OpTag::TermCancel => {
                    *flow
                        .terminal_cancelled
                        .get_mut(slot)
                        .ok_or_else(unsupported)? = true;
                    flow.cancel_checks += 1;
                }
            }
        }
        Ok(flow)
    }
}

/// What one pool thread owns.
struct Local<R: Representation> {
    representation: R,
    /// Partial sums of the terminal accumulators.
    terminals: Vec<R::Weight>,
    ops: usize,
}

struct Pass<'a, R: Representation> {
    trace: &'a Trace,
    flow: &'a Dataflow,
    values: Vec<Mutex<R::Weight>>,
    pending: Vec<AtomicU32>,
    locals: Vec<Mutex<Local<R>>>,
    failed: AtomicBool,
    error: Mutex<Option<ReductionError>>,
    budget: &'a ResourceBudget,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Trace operations between two resource checkpoints of one thread.
const CHECKPOINT_OPS: usize = 64;

impl<'a, R> Pass<'a, R>
where
    R: Summable + Send,
    R::Weight: Send,
{
    fn local(&self) -> std::sync::MutexGuard<'_, Local<R>> {
        let index = rayon::current_thread_index().unwrap_or(0) % self.locals.len();
        lock(&self.locals[index])
    }

    fn fail(&self, error: ReductionError) {
        let mut slot = lock(&self.error);
        if slot.is_none() {
            *slot = Some(error);
        }
        self.failed.store(true, Ordering::Release);
    }

    /// One write into `value` has run.
    fn wrote<'s>(&'s self, scope: &rayon::Scope<'s>, value: u32, local: &mut Local<R>)
    where
        'a: 's,
    {
        if self.pending[value as usize].fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        match self.flow.consumer[value as usize] {
            NONE => {}
            CANCELLED => {
                let weight = std::mem::take(&mut *lock(&self.values[value as usize]));
                let zero = local.representation.is_zero(&weight);
                local.representation.recycle(weight);
                if !zero {
                    self.fail(error(
                        "a key erased as exactly zero in the recording pass is nonzero \
                         in a polynomial replay",
                    ));
                }
            }
            step => scope.spawn(move |scope| {
                crate::prof!(FinReplay);
                self.budget.run_captured(|| self.step(scope, step))
            }),
        }
    }

    fn step<'s>(&'s self, scope: &rayon::Scope<'s>, step: u32)
    where
        'a: 's,
    {
        if self.failed.load(Ordering::Acquire) {
            return;
        }
        let (value, start, end) = self.flow.steps[step as usize];
        let mut active = std::mem::take(&mut *lock(&self.values[value as usize]));
        let mut guard = self.local();
        let local = &mut *guard;
        local.representation.activate(&mut active);
        let result = (|| -> Result<(), ReductionError> {
            for index in start..end {
                let op = self.trace.ops[index as usize];
                match op.tag() {
                    OpTag::New | OpTag::Add => {
                        local.ops += 1;
                        if local.ops.is_multiple_of(CHECKPOINT_OPS) {
                            resources::checkpoint()?;
                        }
                        let target = self.flow.target[index as usize];
                        {
                            let mut destination = lock(&self.values[target as usize]);
                            local.representation.add_product(
                                &mut destination,
                                &active,
                                op.coefficient,
                            )?;
                        }
                        self.wrote(scope, target, local);
                    }
                    OpTag::TermNew | OpTag::TermAdd => {
                        local.ops += 1;
                        let target = self.flow.target[index as usize] as usize;
                        let Local {
                            representation,
                            terminals,
                            ..
                        } = &mut *local;
                        representation.add_product(
                            &mut terminals[target],
                            &active,
                            op.coefficient,
                        )?;
                    }
                    OpTag::Init | OpTag::Pop | OpTag::Cancel | OpTag::TermCancel => {}
                }
            }
            Ok(())
        })();
        local.representation.recycle(active);
        if let Err(error) = result {
            self.fail(error);
        }
    }
}

/// Result of one parallel replay job (see `Replayed`).
pub(super) struct ParReplayed {
    pub images: Vec<Vec<Image>>,
    pub cancel_checks: usize,
}

/// Evaluate the trace with one weight arithmetic per pool thread (`make`)
/// and return the images of `outputs` for each of `lanes` (see the module
/// docs). `None` when the trace shape is not supported (the caller then
/// replays sequentially).
pub(super) fn replay_parallel<R, F>(
    trace: &Trace,
    outputs: &[usize],
    lanes: usize,
    pool: &rayon::ThreadPool,
    budget: &ResourceBudget,
    make: F,
) -> Option<Result<ParReplayed, ReductionError>>
where
    R: Summable + Send,
    R::Weight: Send,
    F: Fn() -> R,
{
    let flow = Dataflow::build(trace).ok()?;
    let threads = pool.current_num_threads().max(1);
    let pass = Pass {
        trace,
        flow: &flow,
        values: (0..flow.writes.len())
            .map(|_| Mutex::new(R::Weight::default()))
            .collect(),
        pending: flow.writes.iter().map(|&w| AtomicU32::new(w)).collect(),
        locals: (0..threads)
            .map(|_| {
                Mutex::new(Local {
                    representation: make(),
                    terminals: (0..trace.terminal_slots)
                        .map(|_| R::Weight::default())
                        .collect(),
                    ops: 0,
                })
            })
            .collect(),
        failed: AtomicBool::new(false),
        error: Mutex::new(None),
        budget,
    };
    pool.install(|| {
        budget.run_captured(|| {
            rayon::scope(|scope| {
                // Every input weight first (no dependencies), then the steps
                // they complete.
                let mut ready = Vec::new();
                {
                    let mut guard = pass.local();
                    for &index in &flow.inits {
                        let op = trace.ops[index as usize];
                        let value = flow.target[index as usize];
                        match guard.representation.coefficient(op.coefficient) {
                            Ok(weight) => *lock(&pass.values[value as usize]) = weight,
                            Err(error) => {
                                pass.fail(error);
                                return;
                            }
                        }
                        if pass.pending[value as usize].fetch_sub(1, Ordering::AcqRel) == 1 {
                            ready.push(value);
                        }
                    }
                }
                for value in ready {
                    let mut guard = pass.local();
                    pass.pending[value as usize].fetch_add(1, Ordering::AcqRel);
                    pass.wrote(scope, value, &mut guard);
                }
            })
        })
    });
    if let Some(error) = lock(&pass.error).take() {
        return Some(Err(error));
    }
    // Every value must have been consumed or be a leftover with no consumer.
    for (value, pending) in pass.pending.iter().enumerate() {
        if pending.load(Ordering::Acquire) != 0 && flow.consumer[value] != NONE {
            return Some(Err(error("a parallel replay left a value incomplete")));
        }
    }
    // Sum the terminal partials of every thread.
    let mut locals: Vec<Local<R>> = pass
        .locals
        .into_iter()
        .map(|local| local.into_inner().unwrap_or_else(|e| e.into_inner()))
        .collect();
    let (first, rest) = locals.split_first_mut().expect("at least one thread");
    for other in rest.iter() {
        for (slot, partial) in other.terminals.iter().enumerate() {
            if let Err(error) = first
                .representation
                .add_weight(&mut first.terminals[slot], partial)
            {
                return Some(Err(error));
            }
        }
    }
    for (slot, cancelled) in flow.terminal_cancelled.iter().enumerate() {
        if *cancelled && !first.representation.is_zero(&first.terminals[slot]) {
            return Some(Err(error(
                "a key erased as exactly zero in the recording pass is nonzero \
                 in a polynomial replay",
            )));
        }
    }
    let images = (0..lanes)
        .map(|lane| {
            outputs
                .iter()
                .map(|&slot| first.representation.image(&first.terminals[slot], lane))
                .collect()
        })
        .collect();
    Some(Ok(ParReplayed {
        images,
        cancel_checks: flow.cancel_checks,
    }))
}
