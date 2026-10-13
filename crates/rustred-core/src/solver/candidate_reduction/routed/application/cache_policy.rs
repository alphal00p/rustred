//! Dependency-safe eviction of completed exact results. No arithmetic or
//! source authority lives here. Metadata contains keys/counts, not coefficients.
use std::collections::BTreeMap;

use super::{CacheEntry, CandidateRoutedCacheCensus, RoutedCandidateApplication};
use crate::algebra::Coefficient;
pub(super) use crate::reduction::cache_policy::{EvictionIndex, EvictionMetadata};
use crate::reduction::cache_policy::{check_entry, is_cache_limit};
use crate::reduction::{CacheWeight, ReductionError, coefficient_cache_weight};
use crate::solver::candidate_reduction::cache::CachedTerms;
use crate::solver::candidate_reduction::routed::CandidateRoutedError;
use crate::solver::candidate_reduction::routed::trace::Node;

/// Backward-compatible routed name for the shared completed-result policy.
pub use crate::reduction::CompletedCachePolicy as CandidateRoutedCachePolicy;

/// Reuse work, not process RSS. Evicted payload includes retained guards.
/// `recomputed_nodes` counts cache misses for nodes already entered in the
/// *same request*. No unbounded history is kept to classify later requests.
/// Cumulative counters saturate, as do the existing reduction work counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedCacheStatistics {
    pub evicted_entries: usize,
    pub evicted_coefficient_terms: usize,
    pub evicted_coefficient_bytes: usize,
    pub recomputed_nodes: usize,
}

impl CandidateRoutedCacheStatistics {
    pub(super) fn merge(&mut self, other: Self) {
        self.evicted_entries = self.evicted_entries.saturating_add(other.evicted_entries);
        self.evicted_coefficient_terms = self
            .evicted_coefficient_terms
            .saturating_add(other.evicted_coefficient_terms);
        self.evicted_coefficient_bytes = self
            .evicted_coefficient_bytes
            .saturating_add(other.evicted_coefficient_bytes);
        self.recomputed_nodes = self.recomputed_nodes.saturating_add(other.recomputed_nodes);
    }
}

#[derive(Default)]
pub(super) struct DependencyPins<const N: usize> {
    // One record per live dependency, bounded by max_unique_nodes; repeated
    // parent references share a checked count instead of duplicating keys.
    nodes: BTreeMap<Node<N>, usize>,
    // Enabled only for an observed evicting request. Each completed entry is counted
    // once, independent of the number of parents that currently reference it.
    pub(super) census: Option<CandidateRoutedCacheCensus>,
}

impl<const N: usize> RoutedCandidateApplication<N> {
    fn eviction_enabled(&self) -> bool {
        self.cache_policy.evicts()
    }

    pub(super) fn pin_dependency(
        &mut self,
        node: &Node<N>,
        pins: &mut DependencyPins<N>,
    ) -> Result<(), CandidateRoutedError> {
        if !self.eviction_enabled() {
            return Ok(());
        }
        if let Some(count) = pins.nodes.get_mut(node) {
            *count = count
                .checked_add(1)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "routed dependency references",
                })?;
        } else {
            if pins.nodes.len() >= self.limits.max_unique_nodes {
                return Err(CandidateRoutedError::ResourceLimit {
                    resource: "pinned operational nodes",
                    requested: pins.nodes.len().saturating_add(1),
                    limit: self.limits.max_unique_nodes,
                });
            }
            pins.nodes.insert(node.clone(), 1);
            if let Some(entry) = self.cache.get(node) {
                if let Some(census) = &mut pins.census {
                    census.retain(entry.weight);
                }
                self.eviction_index.remove(node, entry.eviction);
            }
        }
        Ok(())
    }

    pub(super) fn touch_cache(&mut self, node: &Node<N>) -> Result<(), CandidateRoutedError> {
        self.access_cache(node, false)
    }

    /// Exactly one already-completed result reused by an Enter cache hit.
    pub(super) fn reuse_cache(&mut self, node: &Node<N>) -> Result<(), CandidateRoutedError> {
        self.access_cache(node, true)
    }

    fn access_cache(&mut self, node: &Node<N>, reuse: bool) -> Result<(), CandidateRoutedError> {
        if !self.eviction_enabled() {
            return Ok(());
        }
        let entry = self
            .cache
            .get_mut(node)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "routed cache access refers to an absent result",
            })?;
        let metadata = self
            .eviction_index
            .accessed(self.cache_policy, entry.eviction, reuse)?;
        // Check all scalar changes before altering the entry or eligibility.
        let eligible = self.eviction_index.remove(node, entry.eviction);
        entry.eviction = metadata;
        if eligible {
            self.eviction_index.insert(node.clone(), metadata);
        }
        Ok(())
    }

    pub(super) fn release_dependency(
        &mut self,
        node: &Node<N>,
        pins: &mut DependencyPins<N>,
    ) -> Result<(), CandidateRoutedError> {
        if !self.eviction_enabled() {
            return Ok(());
        }
        let count = pins
            .nodes
            .get_mut(node)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "routed dependency pin is absent at release",
            })?;
        *count = count
            .checked_sub(1)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "routed dependency pin underflowed",
            })?;
        if *count == 0 {
            pins.nodes.remove(node);
            if let Some(entry) = self.cache.get(node) {
                if let Some(census) = &mut pins.census {
                    census.release(entry.weight);
                }
                self.eviction_index.insert(node.clone(), entry.eviction);
            }
        }
        Ok(())
    }

    /// Called on every normal Result exit, including arithmetic, scheduling,
    /// admission and cache errors. Partially computed nodes are never cached.
    pub(super) fn release_request_pins(&mut self, pins: &mut DependencyPins<N>) {
        for (node, _) in std::mem::take(&mut pins.nodes) {
            if let Some(entry) = self.cache.get(&node) {
                self.eviction_index.insert(node, entry.eviction);
            }
        }
        if let Some(census) = &mut pins.census {
            *census = CandidateRoutedCacheCensus::default();
        }
    }

    pub(super) fn cache_insert(
        &mut self,
        node: Node<N>,
        terms: CachedTerms,
        conditions: Vec<Coefficient>,
        pins: &mut DependencyPins<N>,
        stats: &mut CandidateRoutedCacheStatistics,
    ) -> Result<(), CandidateRoutedError> {
        let weight = terms
            .weight()?
            .checked_add(coefficient_cache_weight(&conditions)?)?;
        let evict = self.eviction_enabled();
        // Do not evict useful entries for a result that cannot fit even alone.
        if evict {
            check_entry(weight, self.context.limits)?;
        }
        let stamp = if evict {
            self.eviction_index.next_access()?
        } else {
            0
        };
        loop {
            // Re-evaluate after each victim: admission uses the final global
            // age, and priority overflow must precede budget replacement.
            let eviction = self.eviction_index.admission(self.cache_policy, stamp)?;
            let previous = self.cache.get(&node);
            let previous_weight = previous.map(|entry| entry.weight).unwrap_or_default();
            let prospective = self
                .cache_weight
                .checked_sub(previous_weight)?
                .checked_add(weight)?;
            match self.cache_budget.replace(
                usize::from(previous.is_some()),
                previous_weight,
                1,
                weight,
                self.context.limits,
            ) {
                Ok(()) => {
                    let pinned = evict && pins.nodes.contains_key(&node);
                    if let Some(census) = &mut pins.census {
                        if pinned {
                            if previous.is_some() {
                                census.release(previous_weight);
                            }
                            census.retain(weight);
                        }
                    }
                    if let Some(previous) = previous {
                        self.eviction_index.remove(&node, previous.eviction);
                    }
                    let eligible = (evict && !pinned).then(|| node.clone());
                    self.cache.insert(
                        node,
                        CacheEntry {
                            terms,
                            conditions,
                            weight,
                            eviction,
                        },
                    );
                    self.cache_weight = prospective;
                    if let Some(node) = eligible {
                        self.eviction_index.insert(node, eviction);
                    }
                    return Ok(());
                }
                Err(error) if evict && is_cache_limit(&error) => {
                    let Some((priority, access, victim)) =
                        self.eviction_index.eligible.first().cloned()
                    else {
                        // All remaining entries are protected dependencies.
                        return Err(error.into());
                    };
                    if pins.nodes.contains_key(&victim) {
                        return Err(ReductionError::ReducerInvariant {
                            detail: "a pinned routed result entered the eviction index",
                        }
                        .into());
                    }
                    let victim_entry =
                        self.cache
                            .get(&victim)
                            .ok_or(ReductionError::ReducerInvariant {
                                detail: "routed eviction index refers to an absent result",
                            })?;
                    let victim_weight = victim_entry.weight;
                    let remaining = self.cache_weight.checked_sub(victim_weight)?;
                    // Budget replacement is transactional. Only after its
                    // success remove exactly the corresponding payload/index.
                    self.cache_budget.replace(
                        1,
                        victim_weight,
                        0,
                        CacheWeight::default(),
                        self.context.limits,
                    )?;
                    self.cache.remove(&victim);
                    self.eviction_index.evicted(priority, access, victim);
                    self.cache_weight = remaining;
                    stats.merge(CandidateRoutedCacheStatistics {
                        evicted_entries: 1,
                        evicted_coefficient_terms: victim_weight.coefficient_terms,
                        evicted_coefficient_bytes: victim_weight.coefficient_bytes,
                        recomputed_nodes: 0,
                    });
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests;
