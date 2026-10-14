//! Exact shared guard provenance. One guard pool, local ID lists and direct
//! edges replace quadratic subtree guard-vector copies. Only completed nodes
//! may be flattened; family-qualified IDs prevent lower-family collisions.
use super::GuardedCaptureStatistics;
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::{
    CacheWeight, ReductionError, ReductionLimits, SharedCacheBudget, coefficient_cache_weight,
    guard_union::GuardIndex,
};
use std::collections::HashMap;
use std::sync::Arc;

pub(crate) type GuardNodeId = usize;
const DISABLED: GuardNodeId = usize::MAX;

#[derive(Default)]
struct Node {
    local: Vec<usize>,
    children: Vec<GuardNodeId>,
    complete: bool,
    visited: usize,
}

pub(crate) struct GuardProvenance {
    guards: Vec<Coefficient>,
    index: GuardIndex,
    nodes: Vec<Node>,
    keys: HashMap<(usize, IntegralKey), GuardNodeId>,
    guard_visits: Vec<usize>,
    stack: Vec<GuardNodeId>,
    traversal: usize,
    shared: Arc<SharedCacheBudget>,
    limits: ReductionLimits,
    weight: CacheWeight,
    metadata_limit: usize,
    statistics: GuardedCaptureStatistics,
}

impl GuardProvenance {
    pub(crate) fn new(
        shared: Arc<SharedCacheBudget>,
        limits: ReductionLimits,
        metadata_limit: Option<usize>,
    ) -> Self {
        Self {
            guards: Vec::new(),
            index: GuardIndex::new(),
            nodes: Vec::new(),
            keys: HashMap::new(),
            guard_visits: Vec::new(),
            stack: Vec::new(),
            traversal: 0,
            shared,
            limits,
            weight: CacheWeight::default(),
            metadata_limit: metadata_limit.unwrap_or(limits.max_cached_coefficient_bytes),
            statistics: GuardedCaptureStatistics::default(),
        }
    }
    pub(crate) fn is_active(&self) -> bool {
        !self.statistics.metadata_exhausted
    }
    pub(crate) fn statistics(&self) -> GuardedCaptureStatistics {
        self.statistics
    }
    pub(crate) fn record_captured(&mut self) {
        self.statistics.captured_records += 1;
    }
    pub(crate) fn record_skipped(&mut self) {
        self.statistics.skipped_records += 1;
    }
    pub(crate) fn all_conditions(&self) -> Vec<Coefficient> {
        self.guards.clone()
    }
    pub(crate) fn is_complete(&self, node: GuardNodeId) -> bool {
        !self.is_active() || self.nodes.get(node).is_some_and(|node| node.complete)
    }
    /// Identity is the immutable program/artifact address, never just powers.
    pub(crate) fn node_for(
        &mut self,
        owner: usize,
        key: &IntegralKey,
    ) -> Result<GuardNodeId, ReductionError> {
        if !self.is_active() {
            return Ok(DISABLED);
        }
        if let Some(id) = self.keys.get(&(owner, key.clone())) {
            return Ok(*id);
        }
        // Charge actual hash-table capacity and owned key storage. The extra
        // slot overhead covers load-factor slack and control bytes; no guard
        // polynomial is duplicated in this family-qualified key registry.
        let old = self.keys.capacity();
        self.keys
            .try_reserve(1)
            .map_err(|_| allocation("guard provenance key registry", self.keys.len() + 1))?;
        let table = (self.keys.capacity() - old)
            .checked_mul(std::mem::size_of::<((usize, IntegralKey), usize)>() + 16)
            .ok_or_else(overflow)?;
        let bytes = key
            .powers()
            .len()
            .checked_mul(std::mem::size_of::<i64>())
            .and_then(|v| v.checked_add(table))
            .ok_or_else(overflow)?;
        if !self.reserve_metadata(bytes)? {
            return Ok(DISABLED);
        }
        let id = self.allocate_node()?;
        if self.is_active() {
            self.keys.insert((owner, key.clone()), id);
        }
        Ok(id)
    }
    pub(crate) fn allocate_node(&mut self) -> Result<GuardNodeId, ReductionError> {
        if !self.is_active() {
            return Ok(DISABLED);
        }
        let old = self.nodes.capacity();
        self.nodes
            .try_reserve(1)
            .map_err(|_| allocation("guard provenance nodes", self.nodes.len() + 1))?;
        let bytes = (self.nodes.capacity() - old)
            .checked_mul(std::mem::size_of::<Node>())
            .ok_or_else(overflow)?;
        if !self.reserve_metadata(bytes)? {
            return Ok(DISABLED);
        }
        let id = self.nodes.len();
        self.nodes.push(Node::default());
        self.statistics.nodes += 1;
        Ok(id)
    }
    pub(crate) fn add_local(
        &mut self,
        node: GuardNodeId,
        guards: &[Coefficient],
    ) -> Result<(), ReductionError> {
        if self.is_active() && self.checked(node)?.complete {
            return Err(invariant("cannot change a completed guard provenance node"));
        }
        for guard in guards {
            if guard.is_zero() {
                return Err(invariant("zero captured nonzero condition"));
            }
            let shared = &self.shared;
            let limits = self.limits;
            let weight = &mut self.weight;
            let id = self.index.intern(&mut self.guards, guard, |guard| {
                let next = weight.checked_add(coefficient_cache_weight([guard])?)?;
                shared.replace(0, *weight, 0, next, limits)?;
                *weight = next;
                Ok(())
            })?;
            self.statistics.unique_guards = self.guards.len();
            if !self.is_active() {
                continue;
            }
            // Local repeated guards are usually rare and short; no second
            // coefficient equality operation or duplicated polynomial occurs.
            if self.nodes[node].local.contains(&id) {
                continue;
            }
            let old = self.nodes[node].local.capacity();
            self.nodes[node]
                .local
                .try_reserve(1)
                .map_err(|_| allocation("guard provenance local IDs", old + 1))?;
            let bytes = (self.nodes[node].local.capacity() - old)
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(overflow)?;
            if self.reserve_metadata(bytes)? {
                self.nodes[node].local.push(id);
            }
        }
        Ok(())
    }
    pub(crate) fn add_child(
        &mut self,
        parent: GuardNodeId,
        child: GuardNodeId,
    ) -> Result<(), ReductionError> {
        if !self.is_active() {
            return Ok(());
        }
        self.checked(child)?;
        if parent == child || self.checked(parent)?.complete {
            return Err(invariant("invalid guard provenance edge"));
        }
        if self.nodes[parent].children.contains(&child) {
            return Ok(());
        }
        let old = self.nodes[parent].children.capacity();
        self.nodes[parent]
            .children
            .try_reserve(1)
            .map_err(|_| allocation("guard provenance edges", old + 1))?;
        let bytes = (self.nodes[parent].children.capacity() - old)
            .checked_mul(std::mem::size_of::<usize>())
            .ok_or_else(overflow)?;
        if self.reserve_metadata(bytes)? {
            self.nodes[parent].children.push(child);
            self.statistics.edges += 1;
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self, node: GuardNodeId) -> Result<(), ReductionError> {
        if !self.is_active() {
            return Ok(());
        }
        if self
            .checked(node)?
            .children
            .iter()
            .any(|&id| !self.nodes[id].complete)
        {
            return Err(invariant("guard provenance child is unfinished"));
        }
        self.nodes[node].complete = true;
        Ok(())
    }
    /// Admit reusable flattening scratch before output coefficient work. The
    /// bounded direct-edge count is a conservative stack allowance even when
    /// several parents point to the same child. No per-export hash tables or
    /// growing transitive combinations are retained.
    pub(crate) fn prepare_export(&mut self, node: GuardNodeId) -> Result<bool, ReductionError> {
        if !self.is_active() {
            return Ok(false);
        }
        if !self.checked(node)?.complete {
            return Err(invariant("cannot prepare unfinished guard provenance"));
        }
        let old = self.guard_visits.capacity();
        self.guard_visits
            .try_reserve(self.guards.len().saturating_sub(self.guard_visits.len()))
            .map_err(|_| allocation("guard provenance traversal marks", self.guards.len()))?;
        if !self.reserve_metadata(
            (self.guard_visits.capacity() - old)
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(overflow)?,
        )? {
            return Ok(false);
        }
        self.guard_visits.resize(self.guards.len(), 0);
        let stack_limit = self.statistics.edges.checked_add(1).ok_or_else(overflow)?;
        let old = self.stack.capacity();
        self.stack
            .try_reserve(stack_limit.saturating_sub(self.stack.len()))
            .map_err(|_| allocation("guard provenance traversal stack", stack_limit))?;
        self.reserve_metadata(
            (self.stack.capacity() - old)
                .checked_mul(std::mem::size_of::<GuardNodeId>())
                .ok_or_else(overflow)?,
        )
    }

    /// Stable local-then-child exact union. A diamond is traversed once and
    /// cancellation in final coefficients never discards a visited guard.
    pub(crate) fn conditions(
        &mut self,
        node: GuardNodeId,
    ) -> Result<Vec<Coefficient>, ReductionError> {
        if !self.is_active() || !self.checked(node)?.complete {
            return Err(invariant(
                "cannot export absent or unfinished guard provenance",
            ));
        }
        if self.guard_visits.len() < self.guards.len()
            || self.stack.capacity() < self.statistics.edges + 1
        {
            return Err(invariant("guard export scratch was not admitted"));
        }
        self.traversal = self.traversal.wrapping_add(1);
        if self.traversal == 0 {
            self.guard_visits.fill(0);
            for node in &mut self.nodes {
                node.visited = 0;
            }
            self.traversal = 1;
        }
        let mut output = Vec::new();
        self.stack.clear();
        self.stack.push(node);
        while let Some(id) = self.stack.pop() {
            let node = &mut self.nodes[id];
            if node.visited == self.traversal {
                continue;
            }
            node.visited = self.traversal;
            for &guard in &node.local {
                if self.guard_visits[guard] != self.traversal {
                    self.guard_visits[guard] = self.traversal;
                    output.push(self.guards[guard].clone());
                }
            }
            self.stack.extend(node.children.iter().rev().copied());
        }
        Ok(output)
    }
    fn checked(&self, node: GuardNodeId) -> Result<&Node, ReductionError> {
        self.nodes
            .get(node)
            .ok_or_else(|| invariant("unknown guard provenance node"))
    }
    fn reserve_metadata(&mut self, bytes: usize) -> Result<bool, ReductionError> {
        if !self.is_active() {
            return Ok(false);
        }
        let next = self
            .statistics
            .metadata_bytes
            .checked_add(bytes)
            .ok_or_else(overflow)?;
        if next > self.metadata_limit {
            self.disable()?;
            return Ok(false);
        }
        // Optional metadata has an independent explicit allowance. Charging it
        // against native coefficient capacity would make an otherwise valid
        // reduction fail at its next memo insertion just because capture is on.
        self.statistics.metadata_bytes = next;
        self.statistics.peak_metadata_bytes = self.statistics.peak_metadata_bytes.max(next);
        Ok(true)
    }
    fn disable(&mut self) -> Result<(), ReductionError> {
        self.nodes = Vec::new();
        self.keys = HashMap::new();
        self.stack = Vec::new();
        self.guard_visits = Vec::new();
        self.statistics.metadata_bytes = 0;
        self.statistics.metadata_exhausted = true;
        Ok(())
    }
}
impl Drop for GuardProvenance {
    fn drop(&mut self) {
        let released = self.shared.replace_without_pressure(
            0,
            self.weight,
            0,
            CacheWeight::default(),
            self.limits,
        );
        debug_assert!(released.is_ok(), "guard provenance census did not release");
    }
}
fn invariant(detail: &'static str) -> ReductionError {
    ReductionError::ReducerInvariant { detail }
}
fn overflow() -> ReductionError {
    ReductionError::CacheResourceCountOverflow {
        resource: "guard provenance metadata",
    }
}
fn allocation(resource: &'static str, requested: usize) -> ReductionError {
    ReductionError::AllocationFailure {
        resource,
        requested,
    }
}

#[cfg(test)]
mod tests;
