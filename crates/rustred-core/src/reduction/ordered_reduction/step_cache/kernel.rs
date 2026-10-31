//! Scheduler-side use of an [`OrderedStepCache`]: a kernel wrapper whose
//! `step` replays a record or runs and records the adapter's checked step.
//!
//! The wrapper is transparent for everything else (positions, observation,
//! phase inspection), so the scheduler, its weight arithmetics and their
//! scheduling decisions are unchanged.
//!
//! Concurrent scheduling: when the adapter has a concurrent stepper, the
//! wrapper's stepper looks records up on the worker threads (hits are
//! rebuilt there, misses run the adapter's detached step there). Hit/miss
//! counters, the adapter's accounting and record insertion happen in
//! [`ConcurrentOrderedKernel::absorb`], i.e. at the sequential point of each
//! expanded key, so speculative steps of keys that are never expanded leave
//! no record and the cache content equals that of a sequential run.
use std::collections::BTreeMap;
use std::marker::PhantomData;

use super::codec::{NodeStage, decode_node, encode_node};
use super::{Lookup, OrderedStepCache, OrderedStepShard, StepRecordInput, failure};
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

/// Per-request counters of a cached run.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StepCacheCounters {
    pub hits: usize,
    pub misses: usize,
}

/// Rebuild a recorded step; `key` decodes a recorded child identity.
fn rebuild_with<Key: Ord, E: From<ReductionError>>(
    record: super::RecordView<'_>,
    mut key: impl FnMut(NodeStage, IntegralKey) -> Result<Key, E>,
) -> Result<OrderedStep<Key>, E> {
    let mut terms = Vec::with_capacity(record.terms().len());
    for (node, coefficient) in record.terms() {
        let (stage, integral) = decode_node(node).map_err(failure)?;
        terms.push((key(stage, integral)?, coefficient.clone()));
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

struct CachedKernel<'k, 'c, K> {
    inner: &'k mut K,
    cache: &'c OrderedStepCache,
    counters: StepCacheCounters,
    key: Vec<u8>,
    nodes: Vec<u8>,
}

impl<K: OrderedStepCacheKernel> CachedKernel<'_, '_, K> {
    fn patch(&self, progress: &mut CandidateOrderedReductionProgress) {
        progress.step_cache_hits = self.counters.hits;
        progress.step_cache_misses = self.counters.misses;
    }

    fn rebuild(inner: &K, record: super::RecordView<'_>) -> Result<OrderedStep<K::Key>, K::Error> {
        rebuild_with(record, |stage, integral| {
            inner.step_cache_key(stage, integral)
        })
    }

    /// Offer one computed step; identities without a compact encoding make
    /// the step unrecorded.
    fn record(
        &mut self,
        shard: OrderedStepShard,
        step: &OrderedStep<K::Key>,
        opaque: bool,
    ) -> Result<(), K::Error> {
        self.nodes.clear();
        let mut offsets = Vec::with_capacity(step.terms.len() + step.terminals.len());
        if !opaque {
            for child in step.terms.keys() {
                let Some((stage, integral)) = K::key_view(child) else {
                    self.cache.skip_insert(shard);
                    return Ok(());
                };
                let start = self.nodes.len();
                if !encode_node(stage, integral, &mut self.nodes) {
                    self.cache.skip_insert(shard);
                    return Ok(());
                }
                offsets.push(start..self.nodes.len());
            }
            for terminal in step.terminals.keys() {
                let start = self.nodes.len();
                if !encode_node(OrderedKeyStage::Integral, terminal, &mut self.nodes) {
                    self.cache.skip_insert(shard);
                    return Ok(());
                }
                offsets.push(start..self.nodes.len());
            }
        }
        let empty: &[Coefficient] = &[];
        let nodes = &self.nodes;
        let mut ranges = offsets.into_iter();
        let (terms, terminals, guards) = if opaque {
            (Vec::new(), Vec::new(), empty)
        } else {
            (
                step.terms
                    .values()
                    .map(|coefficient| (&nodes[ranges.next().expect("encoded child")], coefficient))
                    .collect(),
                step.terminals
                    .values()
                    .map(|coefficient| {
                        (
                            &nodes[ranges.next().expect("encoded terminal")],
                            coefficient,
                        )
                    })
                    .collect(),
                step.nonzero_conditions.as_slice(),
            )
        };
        self.cache.insert(
            shard,
            StepRecordInput {
                key: &self.key,
                terms,
                terminals,
                guards,
                opaque,
            },
        )?;
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
                Self::rebuild(inner, record)
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
    let mut cached = CachedKernel {
        inner: kernel,
        cache,
        counters: StepCacheCounters::default(),
        key: Vec::new(),
        nodes: Vec::new(),
    };
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

/// Worker-side cache hooks of an adapter with a concurrent stepper: the
/// shard and the identity decoding of [`OrderedStepCacheKernel`], computed
/// from the (immutable, shared) stepper. Steppers' steps are pure functions
/// of their key, hence always reproducible.
pub(crate) trait ConcurrentStepCacheKernel:
    OrderedStepCacheKernel + ConcurrentOrderedKernel
{
    fn stepper_cache_shard(
        stepper: &Self::Stepper<'_>,
        key: &Self::Key,
    ) -> Option<OrderedStepShard>;
    fn stepper_cache_key(
        stepper: &Self::Stepper<'_>,
        stage: NodeStage,
        integral: IntegralKey,
    ) -> Result<Self::Key, Self::Error>;
}

/// A step computed ahead of time to record when (and only if) it is applied.
pub(crate) struct PendingRecord<Key> {
    shard: OrderedStepShard,
    key: Vec<u8>,
    step: OrderedStep<Key>,
}

/// Account of one cached detached step, applied by `absorb`.
pub(crate) enum CachedAccount<Key, A> {
    /// The adapter does not record this identity.
    Uncached(A),
    /// The identity has no compact encoding: computed, never recorded.
    Unencodable(OrderedStepShard, A),
    /// Replayed from a record.
    Hit,
    /// The lookup or the rebuild of a record failed (the step's error).
    Failed,
    /// Computed by the adapter; `record` is offered to the cache once the
    /// key is expanded.
    Miss {
        account: A,
        record: Option<PendingRecord<Key>>,
    },
}

/// The concurrent stepper of a cached kernel.
pub(crate) struct CachedStepper<'s, K: ConcurrentOrderedKernel + 's> {
    inner: K::Stepper<'s>,
    cache: &'s OrderedStepCache,
    context: &'s CoefficientContext,
    kernel: PhantomData<fn() -> K>,
}

impl<'s, K: ConcurrentStepCacheKernel + 's> OrderedStepper for CachedStepper<'s, K> {
    type Key = K::Key;
    type Priority = K::Priority;
    type Error = K::Error;
    type Account = CachedAccount<K::Key, K::Account>;

    fn position(&self, key: &Self::Key) -> Result<OrderedPosition<Self::Priority>, Self::Error> {
        self.inner.position(key)
    }

    fn step(
        &self,
        key: &Self::Key,
    ) -> (Result<OrderedStep<Self::Key>, Self::Error>, Self::Account) {
        let Some(shard) = K::stepper_cache_shard(&self.inner, key) else {
            let (result, account) = self.inner.step(key);
            return (result, CachedAccount::Uncached(account));
        };
        let mut encoded = Vec::new();
        let encodable = K::key_view(key)
            .is_some_and(|(stage, integral)| encode_node(stage, integral, &mut encoded));
        if !encodable {
            let (result, account) = self.inner.step(key);
            return (result, CachedAccount::Unencodable(shard, account));
        }
        let lookup = self.cache.lookup(shard, &encoded, self.context, |record| {
            rebuild_with(record, |stage, integral| {
                K::stepper_cache_key(&self.inner, stage, integral)
            })
        });
        let record = match lookup {
            Err(error) => return (Err(error.into()), CachedAccount::Failed),
            Ok(Lookup::Hit(Ok(step))) => return (Ok(step), CachedAccount::Hit),
            Ok(Lookup::Hit(Err(error))) => return (Err(error), CachedAccount::Failed),
            Ok(Lookup::Opaque | Lookup::Unavailable) => false,
            Ok(Lookup::Miss) => true,
        };
        let (result, account) = self.inner.step(key);
        let record = match (&result, record) {
            (Ok(step), true) => Some(PendingRecord {
                shard,
                key: encoded,
                step: OrderedStep {
                    terms: step.terms.clone(),
                    terminals: step.terminals.clone(),
                    nonzero_conditions: step.nonzero_conditions.clone(),
                },
            }),
            _ => None,
        };
        (result, CachedAccount::Miss { account, record })
    }

    fn later_phase(&self, parent: &Self::Key, child: &Self::Key) -> bool {
        self.inner.later_phase(parent, child)
    }
}

impl<K: ConcurrentStepCacheKernel> ConcurrentOrderedKernel for CachedKernel<'_, '_, K> {
    type Account = CachedAccount<K::Key, K::Account>;
    type Stepper<'s>
        = CachedStepper<'s, K>
    where
        Self: 's;

    fn stepper(&self) -> Option<CachedStepper<'_, K>> {
        let inner = self.inner.stepper()?;
        Some(CachedStepper {
            inner,
            cache: self.cache,
            context: self.inner.context(),
            kernel: PhantomData,
        })
    }

    /// The sequential cached step's effects, in its order: a hit counts and
    /// charges the replayed step; a miss counts, applies the adapter's
    /// account and records the step.
    fn absorb(
        &mut self,
        key: &Self::Key,
        account: Self::Account,
        request: &mut ReductionRequest,
        work: &mut ReductionStatistics,
    ) -> Result<(), Self::Error> {
        match account {
            CachedAccount::Uncached(account) => self.inner.absorb(key, account, request, work),
            CachedAccount::Unencodable(shard, account) => {
                self.cache.skip_insert(shard);
                self.inner.absorb(key, account, request, work)
            }
            CachedAccount::Hit => {
                self.counters.hits += 1;
                self.inner.step_cache_hit(key)
            }
            CachedAccount::Failed => Ok(()),
            CachedAccount::Miss { account, record } => {
                self.counters.misses += 1;
                self.inner.absorb(key, account, request, work)?;
                if let Some(record) = record {
                    self.key = record.key;
                    self.record(record.shard, &record.step, false)?;
                }
                Ok(())
            }
        }
    }
}

/// [`run_with_step_cache`] with scheduler options: concurrent windows look
/// records up and compute misses on the workers; `scheduler.threads == 1`
/// is exactly [`run_with_step_cache`].
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
    let mut cached = CachedKernel {
        inner: kernel,
        cache,
        counters: StepCacheCounters::default(),
        key: Vec::new(),
        nodes: Vec::new(),
    };
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
