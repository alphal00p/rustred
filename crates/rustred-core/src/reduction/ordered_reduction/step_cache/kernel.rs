//! Scheduler-side use of an [`OrderedStepCache`]: a kernel wrapper whose
//! `step` replays a record or runs and records the adapter's checked step.
//!
//! The wrapper is transparent for everything else (positions, observation,
//! phase inspection), so the scheduler, its weight arithmetics and their
//! scheduling decisions are unchanged.
//!
//! Concurrent adapters ([`ConcurrentStepCacheKernel`]) keep their cached
//! requests on the scheduler's workers: the wrapper's stepper
//! ([`CachedStepper`]) replays a record or runs the adapter's stepper on a
//! worker, possibly ahead of time and for a key that later cancels; it
//! changes no record and no counter. At the key's sequential turn the wrapper's
//! `absorb` counts the lookup, applies the hit accounting or the adapter's
//! step account, and inserts the new record. Exactly the keys the
//! sequential scheduler expands are counted and recorded, in its order, so
//! the records, the shard counters (hits, misses, inserts) and the request's
//! hit and miss counters are those of the sequential run for every thread
//! count. Only shard loading can differ: a worker may load a shard for a key
//! that then cancels, and under a resident-byte ceiling or a memory budget
//! the earlier load can change which shards and records fit.
use std::collections::BTreeMap;
use std::ops::Range;

use super::codec::{NodeStage, decode_node, encode_node};
use super::{
    Lookup, LookupCount, OrderedStepCache, OrderedStepShard, RecordView, StepRecordInput, failure,
};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::ordered_reduction::{
    ConcurrentOrderedKernel, OrderedKernel, OrderedKeyStage, OrderedOutput, OrderedPhaseEvent,
    OrderedPosition, OrderedSchedulerOptions, OrderedStep, OrderedStepper, OrderedWeightArithmetic,
    run_with_arithmetic, run_with_options,
};
use crate::reduction::{ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics};
use crate::solver::CandidateOrderedReductionProgress;

/// Adapter hooks needed to record and replay its checked steps.
pub(crate) trait OrderedStepCacheKernel: OrderedKernel {
    /// The shard of `key`'s record, or `None` for an identity this adapter
    /// never records.
    fn step_cache_shard(&mut self, key: &Self::Key) -> Option<OrderedStepShard>;
    /// Rebuild a scheduled identity from its stored stage and integral.
    fn step_cache_key(
        &self,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<Self::Key, Self::Error>;
    /// Whether the most recent `step` was a function of its key alone. An
    /// adapter returns `false` after a step whose output depends on earlier
    /// steps of the request; such a step is recorded as opaque.
    fn step_was_reproducible(&self) -> bool {
        true
    }
    /// Per-step accounting and cancellation checks that a replayed step must
    /// still perform (e.g. operational-node limits).
    fn step_cache_hit(&mut self, _key: &Self::Key) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The cache hooks of a concurrent adapter, evaluated on the scheduler's
/// workers through its shared stepper (the kernel itself stays on the
/// calling thread). They must agree with the kernel's
/// [`OrderedStepCacheKernel`] hooks. Stepper steps are pure functions of the
/// key, so they are always reproducible (never opaque); a hit is accounted
/// by [`OrderedStepCacheKernel::step_cache_hit`] at the key's turn.
pub(crate) trait ConcurrentStepCacheKernel:
    OrderedStepCacheKernel + ConcurrentOrderedKernel
{
    /// [`OrderedStepCacheKernel::step_cache_shard`] through the stepper.
    fn stepper_shard<'s>(stepper: &Self::Stepper<'s>, key: &Self::Key) -> Option<OrderedStepShard>
    where
        Self: 's;
    /// [`OrderedStepCacheKernel::step_cache_key`] through the stepper.
    fn stepper_key<'s>(
        stepper: &Self::Stepper<'s>,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<Self::Key, Self::Error>
    where
        Self: 's;
}

/// Per-request counters of a cached run.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StepCacheCounters {
    pub hits: usize,
    pub misses: usize,
}

struct CachedKernel<'k, 'c, K> {
    inner: &'k mut K,
    cache: &'c OrderedStepCache,
    counters: StepCacheCounters,
    key: Vec<u8>,
    nodes: Vec<u8>,
}

/// Rebuild a recorded step; `decode` turns a stored identity back into a
/// scheduled key.
fn rebuild<Key: Ord, E: From<ReductionError>>(
    record: RecordView<'_>,
    decode: impl Fn(NodeStage, IntegralKey) -> Result<Key, E>,
) -> Result<OrderedStep<Key>, E> {
    let mut terms = Vec::with_capacity(record.terms().len());
    for (node, coefficient) in record.terms() {
        let (stage, integral) = decode_node(node).map_err(failure)?;
        terms.push((decode(stage, integral)?, coefficient.clone()));
    }
    let mut terminals = Vec::with_capacity(record.terminals().len());
    for (node, coefficient) in record.terminals() {
        let (stage, integral) = decode_node(node).map_err(failure)?;
        if stage != NodeStage::Integral {
            return Err(failure("a recorded terminal is not a plain integral").into());
        }
        terminals.push((integral, coefficient.clone()));
    }
    Ok(OrderedStep {
        terms: BTreeMap::from_iter(terms),
        terminals: BTreeMap::from_iter(terminals),
        nonzero_conditions: record.guards().cloned().collect(),
    })
}

/// Encode the children and then the terminals of `step` into `nodes`, one
/// range each; `None` when one of them has no compact encoding (the step is
/// then not recorded).
fn encode_step<K: OrderedKernel>(
    step: &OrderedStep<K::Key>,
    nodes: &mut Vec<u8>,
) -> Option<Vec<Range<usize>>> {
    nodes.clear();
    let mut offsets = Vec::with_capacity(step.terms.len() + step.terminals.len());
    for child in step.terms.keys() {
        let (stage, integral) = K::key_view(child)?;
        let start = nodes.len();
        if !encode_node(stage, integral, nodes) {
            return None;
        }
        offsets.push(start..nodes.len());
    }
    for terminal in step.terminals.keys() {
        let start = nodes.len();
        if !encode_node(OrderedKeyStage::Integral, terminal, nodes) {
            return None;
        }
        offsets.push(start..nodes.len());
    }
    Some(offsets)
}

/// The record of a step whose children and terminals are encoded in
/// `nodes` at `offsets` (children first).
fn record_input<'a>(
    key: &'a [u8],
    nodes: &'a [u8],
    offsets: &[Range<usize>],
    terms: impl Iterator<Item = &'a Coefficient>,
    terminals: impl Iterator<Item = &'a Coefficient>,
    guards: &'a [Coefficient],
) -> StepRecordInput<'a> {
    let mut ranges = offsets.iter();
    let mut node = || &nodes[ranges.next().expect("encoded child or terminal").clone()];
    let terms = terms.map(|coefficient| (node(), coefficient)).collect();
    let terminals = terminals.map(|coefficient| (node(), coefficient)).collect();
    StepRecordInput {
        key,
        terms,
        terminals,
        guards,
        opaque: false,
    }
}

impl<'k, 'c, K: OrderedStepCacheKernel> CachedKernel<'k, 'c, K> {
    fn new(inner: &'k mut K, cache: &'c OrderedStepCache) -> Self {
        Self {
            inner,
            cache,
            counters: StepCacheCounters::default(),
            key: Vec::new(),
            nodes: Vec::new(),
        }
    }

    fn patch(&self, progress: &mut CandidateOrderedReductionProgress) {
        progress.step_cache_hits = self.counters.hits;
        progress.step_cache_misses = self.counters.misses;
    }

    /// Offer one computed step; identities without a compact encoding make
    /// the step unrecorded.
    fn record(
        &mut self,
        shard: OrderedStepShard,
        step: &OrderedStep<K::Key>,
        opaque: bool,
    ) -> Result<(), K::Error> {
        let record = if opaque {
            StepRecordInput {
                key: &self.key,
                terms: Vec::new(),
                terminals: Vec::new(),
                guards: &[],
                opaque: true,
            }
        } else {
            let Some(offsets) = encode_step::<K>(step, &mut self.nodes) else {
                self.cache.skip_insert(shard);
                return Ok(());
            };
            record_input(
                &self.key,
                &self.nodes,
                &offsets,
                step.terms.values(),
                step.terminals.values(),
                &step.nonzero_conditions,
            )
        };
        self.cache.insert(shard, record)?;
        Ok(())
    }
}

impl<K: OrderedStepCacheKernel> OrderedKernel for CachedKernel<'_, '_, K> {
    type Key = K::Key;
    type Priority = K::Priority;
    type Error = K::Error;

    fn context(&self) -> &CoefficientContext {
        self.inner.context()
    }
    fn terminal_composer(
        &self,
    ) -> Option<std::sync::Arc<dyn crate::reduction::OrderedTerminalComposer>> {
        self.inner.terminal_composer()
    }
    fn limits(&self) -> ReductionLimits {
        self.inner.limits()
    }
    fn phase_count(&self) -> usize {
        self.inner.phase_count()
    }
    fn position(
        &mut self,
        key: &Self::Key,
    ) -> Result<OrderedPosition<Self::Priority>, Self::Error> {
        self.inner.position(key)
    }

    fn step(
        &mut self,
        key: &Self::Key,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<OrderedStep<Self::Key>, Self::Error> {
        let Some(shard) = self.inner.step_cache_shard(key) else {
            return self.inner.step(key, request, work);
        };
        self.key.clear();
        let encoded = K::key_view(key)
            .is_some_and(|(stage, integral)| encode_node(stage, integral, &mut self.key));
        if !encoded {
            self.cache.skip_insert(shard);
            return self.inner.step(key, request, work);
        }
        let inner = &*self.inner;
        let lookup = self
            .cache
            .lookup(shard, &self.key, inner.context(), |record| {
                rebuild(record, |stage, integral| {
                    inner.step_cache_key(stage, integral)
                })
            })?;
        let record = match lookup {
            Lookup::Hit(step) => {
                let step = step?;
                self.counters.hits += 1;
                self.inner.step_cache_hit(key)?;
                return Ok(step);
            }
            Lookup::Opaque | Lookup::Unavailable => false,
            Lookup::Miss => true,
        };
        self.counters.misses += 1;
        let step = self.inner.step(key, request, work)?;
        if record {
            let opaque = !self.inner.step_was_reproducible();
            self.record(shard, &step, opaque)?;
        }
        Ok(step)
    }

    fn observe(
        &mut self,
        progress: &CandidateOrderedReductionProgress,
        current: Option<&Self::Key>,
    ) -> Result<(), Self::Error> {
        let mut progress = *progress;
        self.patch(&mut progress);
        self.inner.observe(&progress, current)
    }

    fn record_work(&mut self, work: ReductionStatistics) {
        self.inner.record_work(work);
    }

    fn key_view(key: &Self::Key) -> Option<(OrderedKeyStage<'_>, &IntegralKey)> {
        K::key_view(key)
    }

    fn inspect_phase(&mut self, mut event: OrderedPhaseEvent<'_>) -> Result<(), Self::Error> {
        self.patch(&mut event.progress);
        self.inner.inspect_phase(event)
    }

    /// The cache's tail table.
    fn tail_table(
        &mut self,
    ) -> Result<Option<std::sync::Arc<crate::reduction::OrderedTailTable>>, Self::Error> {
        Ok(self.cache.tail_table(self.inner.context())?)
    }

    fn tail_key(&self, encoded: &[u8]) -> Option<Self::Key> {
        let (stage, integral) = decode_node(encoded).ok()?;
        self.inner.step_cache_key(stage, integral).ok()
    }

    fn tail_encode(key: &Self::Key, out: &mut Vec<u8>) -> bool {
        K::tail_encode(key, out)
    }
}

/// The adapter's stepper behind the cache, shared by the scheduler's
/// workers. See the module docs: a step replays the key's record or runs
/// the adapter's step (loading the key's shard if needed), and leaves every
/// counter and insert to [`ConcurrentOrderedKernel::absorb`] at the key's
/// sequential turn.
pub(crate) struct CachedStepper<'s, K: ConcurrentStepCacheKernel + 's> {
    inner: K::Stepper<'s>,
    cache: &'s OrderedStepCache,
    context: &'s CoefficientContext,
}

/// A computed step's record, encoded on a worker and inserted at the key's
/// turn.
pub(crate) struct PendingRecord {
    key: Vec<u8>,
    nodes: Vec<u8>,
    offsets: Vec<Range<usize>>,
    terms: Vec<Coefficient>,
    terminals: Vec<Coefficient>,
    guards: Vec<Coefficient>,
}

impl PendingRecord {
    fn input(&self) -> StepRecordInput<'_> {
        record_input(
            &self.key,
            &self.nodes,
            &self.offsets,
            self.terms.iter(),
            self.terminals.iter(),
            &self.guards,
        )
    }
}

/// What a computed step offers the cache at its turn.
pub(crate) enum Recording {
    /// Nothing: the step failed, the record is opaque or the shard is
    /// skipped, or the cache does not populate.
    Nothing,
    /// A child or terminal has no compact encoding (a skipped insert).
    Skip,
    Insert(PendingRecord),
}

/// What the cached stepper did for one key, applied by `absorb` at the
/// key's sequential turn exactly as the sequential cached step would have.
pub(crate) enum CachedAccount<A> {
    /// The adapter records no step for this key: its own step and account.
    Uncovered(A),
    /// The key has no compact encoding: the adapter's step; the shard counts
    /// a skipped insert.
    Unencoded(OrderedStepShard, A),
    /// The lookup failed (rejected shard). Its error is the step's result;
    /// nothing is accounted.
    Failed,
    /// The key's record was found. `replayed` is false when the record could
    /// not be rebuilt; that error is then the step's result.
    Hit {
        shard: OrderedStepShard,
        replayed: bool,
    },
    /// No usable record: the adapter's step ran.
    Computed {
        shard: OrderedStepShard,
        count: LookupCount,
        account: A,
        record: Recording,
    },
}

impl<'s, K: ConcurrentStepCacheKernel + 's> OrderedStepper for CachedStepper<'s, K> {
    type Key = K::Key;
    type Priority = K::Priority;
    type Error = K::Error;
    type Account = CachedAccount<K::Account>;

    fn position(&self, key: &K::Key) -> Result<OrderedPosition<K::Priority>, K::Error> {
        self.inner.position(key)
    }

    fn step(
        &self,
        key: &K::Key,
    ) -> (
        Result<OrderedStep<K::Key>, K::Error>,
        CachedAccount<K::Account>,
    ) {
        let Some(shard) = K::stepper_shard(&self.inner, key) else {
            let (result, account) = self.inner.step(key);
            return (result, CachedAccount::Uncovered(account));
        };
        let mut encoded = Vec::new();
        if !K::key_view(key)
            .is_some_and(|(stage, integral)| encode_node(stage, integral, &mut encoded))
        {
            let (result, account) = self.inner.step(key);
            return (result, CachedAccount::Unencoded(shard, account));
        }
        let inner = &self.inner;
        let found = self.cache.probe(shard, &encoded, self.context, |record| {
            rebuild(record, |stage, integral| {
                K::stepper_key(inner, stage, integral)
            })
        });
        let found = match found {
            Ok(found) => found,
            Err(error) => return (Err(error.into()), CachedAccount::Failed),
        };
        let count = found.count();
        let offer = match found {
            Lookup::Hit(step) => {
                let replayed = step.is_ok();
                return (step, CachedAccount::Hit { shard, replayed });
            }
            Lookup::Opaque | Lookup::Unavailable => false,
            Lookup::Miss => true,
        };
        let (result, account) = self.inner.step(key);
        let record = match &result {
            Ok(step) if offer => {
                let mut nodes = Vec::new();
                match encode_step::<K>(step, &mut nodes) {
                    None => Recording::Skip,
                    // A read-only cache stores nothing (`insert` declines).
                    Some(_) if !self.cache.options().populate => Recording::Nothing,
                    Some(offsets) => Recording::Insert(PendingRecord {
                        key: encoded,
                        nodes,
                        offsets,
                        terms: step.terms.values().cloned().collect(),
                        terminals: step.terminals.values().cloned().collect(),
                        guards: step.nonzero_conditions.clone(),
                    }),
                }
            }
            _ => Recording::Nothing,
        };
        (
            result,
            CachedAccount::Computed {
                shard,
                count,
                account,
                record,
            },
        )
    }

    fn later_phase(&self, parent: &K::Key, child: &K::Key) -> bool {
        self.inner.later_phase(parent, child)
    }
}

impl<'k, 'c, K: ConcurrentStepCacheKernel> ConcurrentOrderedKernel for CachedKernel<'k, 'c, K> {
    type Account = CachedAccount<K::Account>;
    type Stepper<'s>
        = CachedStepper<'s, K>
    where
        Self: 's;

    fn stepper(&self) -> Option<CachedStepper<'_, K>> {
        Some(CachedStepper {
            inner: self.inner.stepper()?,
            cache: self.cache,
            context: self.inner.context(),
        })
    }

    /// The cache effects of the sequential cached step, in its order: the
    /// lookup's shard counter, the request's hit or miss, the hit accounting
    /// or the adapter's step account, then the insert of a new record.
    fn absorb(
        &mut self,
        key: &K::Key,
        account: CachedAccount<K::Account>,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), K::Error> {
        match account {
            CachedAccount::Uncovered(account) => self.inner.absorb(key, account, request, work),
            CachedAccount::Unencoded(shard, account) => {
                self.cache.skip_insert(shard);
                self.inner.absorb(key, account, request, work)
            }
            CachedAccount::Failed => Ok(()),
            CachedAccount::Hit { shard, replayed } => {
                self.cache.count_lookup(shard, LookupCount::Hit);
                if !replayed {
                    return Ok(());
                }
                self.counters.hits += 1;
                self.inner.step_cache_hit(key)
            }
            CachedAccount::Computed {
                shard,
                count,
                account,
                record,
            } => {
                self.cache.count_lookup(shard, count);
                self.counters.misses += 1;
                self.inner.absorb(key, account, request, work)?;
                match record {
                    Recording::Nothing => {}
                    Recording::Skip => self.cache.skip_insert(shard),
                    Recording::Insert(record) => {
                        self.cache.insert(shard, record.input())?;
                    }
                }
                Ok(())
            }
        }
    }
}

/// Run one request, replaying and recording checked steps through `cache`
/// when one is attached. Without a cache this is `run_with_arithmetic`.
/// The returned progress carries the request's cache hits and misses.
pub(crate) fn run_with_step_cache<K: OrderedStepCacheKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    cache: Option<&OrderedStepCache>,
) -> Result<OrderedOutput, K::Error> {
    let Some(cache) = cache else {
        return run_with_arithmetic(kernel, request, initial, nonzero_conditions, arithmetic);
    };
    let mut cached = CachedKernel::new(kernel, cache);
    let mut output = run_with_arithmetic(
        &mut cached,
        request,
        initial,
        nonzero_conditions,
        arithmetic,
    )?;
    cached.patch(&mut output.progress);
    Ok(output)
}

/// [`run_with_step_cache`] for a concurrent adapter, with scheduler options:
/// without a cache this is `run_with_options`, and a cached request runs on
/// the scheduler's workers as well (see the module docs). Every thread count
/// returns the same result, progress and observations, and leaves the same
/// records and shard counters in `cache`; `scheduler.threads == 1` is the
/// sequential cached run of [`run_with_step_cache`].
pub(crate) fn run_with_step_cache_and_options<K: ConcurrentStepCacheKernel>(
    kernel: &mut K,
    request: &mut ReductionRequest,
    initial: BTreeMap<K::Key, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    arithmetic: OrderedWeightArithmetic,
    scheduler: OrderedSchedulerOptions,
    cache: Option<&OrderedStepCache>,
) -> Result<OrderedOutput, K::Error> {
    let Some(cache) = cache else {
        return run_with_options(
            kernel,
            request,
            initial,
            nonzero_conditions,
            arithmetic,
            scheduler,
        );
    };
    let mut cached = CachedKernel::new(kernel, cache);
    let mut output = run_with_options(
        &mut cached,
        request,
        initial,
        nonzero_conditions,
        arithmetic,
        scheduler,
    )?;
    cached.patch(&mut output.progress);
    Ok(output)
}
