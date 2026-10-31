//! Scheduler-side use of an [`OrderedStepCache`]: a kernel wrapper whose
//! `step` replays a record or runs and records the adapter's checked step.
//!
//! The wrapper is transparent for everything else (positions, observation,
//! phase inspection), so the scheduler, its weight arithmetics and their
//! scheduling decisions are unchanged.
use std::collections::BTreeMap;

use super::codec::{NodeStage, decode_node, encode_node};
use super::{Lookup, OrderedStepCache, OrderedStepShard, StepRecordInput, failure};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::ordered_reduction::{
    OrderedKernel, OrderedKeyStage, OrderedOutput, OrderedPhaseEvent, OrderedPosition, OrderedStep,
    OrderedWeightArithmetic, run_with_arithmetic,
};
use crate::reduction::{ReductionLimits, ReductionRequest, ReductionStatistics};
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
        let mut terms = Vec::with_capacity(record.terms().len());
        for (node, coefficient) in record.terms() {
            let (stage, integral) = decode_node(node).map_err(failure)?;
            terms.push((inner.step_cache_key(stage, integral)?, coefficient.clone()));
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
