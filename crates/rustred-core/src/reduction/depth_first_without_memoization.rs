//! Occurrence-local DFS over checked actions, without completed-result reuse.
//!
//! A child's unit-weight value is consumed by its parent and then discarded.
//! Only active ancestors are indexed; no completed identity is retained. This
//! scheduler neither asks for an ordering position nor groups pending branches.

use std::collections::{BTreeMap, BTreeSet};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::reduction::guard_union::GuardIndex;
use crate::reduction::{
    ReductionError, ReductionLimits, ReductionRequest, ReductionStatistics,
    accumulate_master_in_request,
};

/// Children retain the checked action's original occurrence order, including
/// duplicate identities. A terminal row is already complete for this adapter.
pub(crate) struct DepthFirstStep<K> {
    pub terms: Vec<(K, Coefficient)>,
    pub terminals: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
}

impl<K> Default for DepthFirstStep<K> {
    fn default() -> Self {
        Self {
            terms: vec![],
            terminals: BTreeMap::new(),
            nonzero_conditions: vec![],
        }
    }
}

pub(crate) trait DepthFirstKernel {
    type Key: Clone + Ord;
    type Error: From<ReductionError>;

    fn context(&self) -> &CoefficientContext;
    fn limits(&self) -> ReductionLimits;
    fn request(&mut self) -> &mut ReductionRequest;
    fn step(
        &mut self,
        key: &Self::Key,
        work: &mut ReductionStatistics,
    ) -> Result<DepthFirstStep<Self::Key>, Self::Error>;
    fn cycle_error(&self, key: &Self::Key) -> Self::Error;
    fn check_cancel(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn completed(&mut self, _key: &Self::Key, _terminal_keys: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Preserve attempted work on both successful and unsuccessful requests.
    fn record_work(&mut self, _work: ReductionStatistics) {}
}

pub(crate) struct DepthFirstOutput {
    pub terms: BTreeMap<IntegralKey, Coefficient>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: ReductionStatistics,
}

struct Value {
    terms: BTreeMap<IntegralKey, Coefficient>,
    conditions: Vec<Coefficient>,
}

struct Frame<K> {
    key: K,
    children: std::vec::IntoIter<(K, Coefficient)>,
    waiting_weight: Option<Coefficient>,
    value: Value,
    guards: GuardIndex,
}

fn invariant(detail: &'static str) -> ReductionError {
    ReductionError::ReducerInvariant { detail }
}

/// All original inputs, even zero weights and canceled duplicates, must already
/// have been validated by the adapter. `initial` preserves its ordinary DFS root
/// order and existing input coalescing. No new frontier coalescing occurs here.
pub(crate) fn run<K: DepthFirstKernel>(
    kernel: &mut K,
    initial: Vec<(K::Key, Coefficient)>,
    nonzero_conditions: Vec<Coefficient>,
) -> Result<DepthFirstOutput, K::Error> {
    let baseline = kernel.request().pending_frame_count();
    let mut work = ReductionStatistics::default();
    let result = run_inner(kernel, initial, nonzero_conditions, &mut work);
    kernel.request().restore_pending_frames(baseline);
    kernel.record_work(work);
    result
}

fn run_inner<K: DepthFirstKernel>(
    kernel: &mut K,
    initial: Vec<(K::Key, Coefficient)>,
    initial_conditions: Vec<Coefficient>,
    work: &mut ReductionStatistics,
) -> Result<DepthFirstOutput, K::Error> {
    let context = kernel.context().clone();
    let limits = kernel.limits();
    let mut conditions = Vec::new();
    let mut guards = GuardIndex::new();
    for condition in &initial_conditions {
        guards.intern(&mut conditions, condition, |_| Ok(()))?;
    }
    let mut output = BTreeMap::new();
    let mut active = BTreeSet::new();
    let mut stack = Vec::<Frame<K::Key>>::new();
    for (root, root_weight) in initial {
        if root_weight.is_zero() {
            continue;
        }
        let mut next = Some(root);
        let mut returned: Option<Value> = None;
        loop {
            symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
            kernel.check_cancel()?;
            if let Some(key) = next.take() {
                if !active.insert(key.clone()) {
                    return Err(kernel.cycle_error(&key));
                }
                kernel
                    .request()
                    .retain_pending_frame(limits.diagnostic_limit(limits.max_pending_frames))?;
                let step = kernel.step(&key, work)?;
                let mut local_conditions = Vec::new();
                let mut local_guards = GuardIndex::new();
                for condition in &step.nonzero_conditions {
                    local_guards.intern(&mut local_conditions, condition, |_| Ok(()))?;
                }
                stack
                    .try_reserve(1)
                    .map_err(|_| ReductionError::AllocationFailure {
                        resource: "non-memoizing DFS frames",
                        requested: stack.len().saturating_add(1),
                    })?;
                stack.push(Frame {
                    key,
                    children: step.terms.into_iter(),
                    waiting_weight: None,
                    value: Value {
                        terms: step.terminals,
                        conditions: local_conditions,
                    },
                    guards: local_guards,
                });
            }
            let frame = stack
                .last_mut()
                .ok_or_else(|| invariant("non-memoizing DFS has no receiving frame"))?;
            if let Some(child) = returned.take() {
                let weight = frame.waiting_weight.take().ok_or_else(|| {
                    invariant("non-memoizing DFS child has no waiting edge coefficient")
                })?;
                for condition in &child.conditions {
                    frame
                        .guards
                        .intern(&mut frame.value.conditions, condition, |_| Ok(()))?;
                }
                for (terminal, coefficient) in child.terms {
                    kernel.check_cancel()?;
                    let contribution = context
                        .try_mul(&weight, &coefficient, limits.exact_algebra)
                        .map_err(ReductionError::from)?;
                    accumulate_master_in_request(
                        &context,
                        &mut frame.value.terms,
                        &terminal,
                        contribution,
                        limits,
                        kernel.request(),
                        work,
                    )?;
                }
            }
            if let Some((child, weight)) = frame.children.next() {
                frame.waiting_weight = Some(weight);
                next = Some(child);
                continue;
            }
            let frame = stack.pop().expect("the current frame exists");
            if !active.remove(&frame.key) {
                return Err(invariant("non-memoizing DFS completed a non-active key").into());
            }
            kernel.request().release_pending_frame()?;
            kernel.completed(&frame.key, frame.value.terms.len())?;
            if stack.is_empty() {
                for condition in &frame.value.conditions {
                    guards.intern(&mut conditions, condition, |_| Ok(()))?;
                }
                for (terminal, coefficient) in frame.value.terms {
                    kernel.check_cancel()?;
                    let contribution = context
                        .try_mul(&root_weight, &coefficient, limits.exact_algebra)
                        .map_err(ReductionError::from)?;
                    accumulate_master_in_request(
                        &context,
                        &mut output,
                        &terminal,
                        contribution,
                        limits,
                        kernel.request(),
                        work,
                    )?;
                }
                break;
            }
            returned = Some(frame.value);
        }
    }
    symbolica::utils::resources::checkpoint().map_err(ReductionError::from)?;
    kernel.check_cancel()?;
    Ok(DepthFirstOutput {
        terms: output,
        nonzero_conditions: conditions,
        work: *work,
    })
}

#[cfg(test)]
mod tests;
