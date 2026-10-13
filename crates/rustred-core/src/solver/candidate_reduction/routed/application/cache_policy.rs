//! Dependency-safe eviction of completed exact results. No arithmetic or
//! source authority lives here. Metadata contains keys/counts, not coefficients.
use std::collections::{BTreeMap, BTreeSet};

use super::{CacheEntry, CandidateRoutedCacheCensus, RoutedCandidateApplication};
use crate::algebra::Coefficient;
use crate::reduction::{CacheWeight, ReductionError, ReductionLimits, coefficient_cache_weight};
use crate::solver::candidate_reduction::cache::CachedTerms;
use crate::solver::candidate_reduction::routed::CandidateRoutedError;
use crate::solver::candidate_reduction::routed::trace::Node;

/// Operational cache policy; coefficient and guard limits remain unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateRoutedCachePolicy {
    /// Preserve completed entries and fail when the next entry exceeds a cap.
    #[default]
    FailOnLimit,
    /// Evict least-recently-used completed entries not needed by pending
    /// combines or the current root. A protected working set (or one entry)
    /// that cannot fit still returns the existing typed cache-limit error.
    EvictLeastRecentlyUsed,
}

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

#[derive(Debug, Default)]
pub(super) struct EvictionIndex<const N: usize> {
    // Exactly one record per eligible entry. Pinned entries have no record.
    // Stamps are unique and checked; there is no stale-record priority queue.
    eligible: BTreeSet<(u64, Node<N>)>,
    clock: u64,
}

impl<const N: usize> EvictionIndex<N> {
    fn next_access(&mut self) -> Result<u64, ReductionError> {
        self.clock =
            self.clock
                .checked_add(1)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "routed cache access sequence",
                })?;
        Ok(self.clock)
    }
}

#[derive(Default)]
pub(super) struct DependencyPins<const N: usize> {
    // One record per live dependency, bounded by max_unique_nodes; repeated
    // parent references share a checked count instead of duplicating keys.
    nodes: BTreeMap<Node<N>, usize>,
    // Enabled only for an observed LRU request. Each completed entry is counted
    // once, independent of the number of parents that currently reference it.
    pub(super) census: Option<CandidateRoutedCacheCensus>,
}

fn check_entry(weight: CacheWeight, limits: ReductionLimits) -> Result<(), ReductionError> {
    if limits.max_cached_integrals == 0 {
        return Err(ReductionError::CacheLimit {
            requested: 1,
            limit: 0,
        });
    }
    if weight.coefficient_terms > limits.max_cached_coefficient_terms {
        return Err(ReductionError::CacheCoefficientTermLimit {
            requested: weight.coefficient_terms,
            limit: limits.max_cached_coefficient_terms,
        });
    }
    if weight.coefficient_bytes > limits.max_cached_coefficient_bytes {
        return Err(ReductionError::CacheCoefficientByteLimit {
            requested: weight.coefficient_bytes,
            limit: limits.max_cached_coefficient_bytes,
        });
    }
    Ok(())
}

fn is_cache_limit(error: &ReductionError) -> bool {
    matches!(
        error,
        ReductionError::CacheLimit { .. }
            | ReductionError::CacheCoefficientTermLimit { .. }
            | ReductionError::CacheCoefficientByteLimit { .. }
    )
}

impl<const N: usize> RoutedCandidateApplication<N> {
    fn eviction_enabled(&self) -> bool {
        self.cache_policy == CandidateRoutedCachePolicy::EvictLeastRecentlyUsed
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
                self.eviction_index
                    .eligible
                    .remove(&(entry.last_access, node.clone()));
            }
        }
        Ok(())
    }

    pub(super) fn touch_cache(&mut self, node: &Node<N>) -> Result<(), CandidateRoutedError> {
        if !self.eviction_enabled() {
            return Ok(());
        }
        let stamp = self.eviction_index.next_access()?;
        let entry = self
            .cache
            .get_mut(node)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "routed LRU access refers to an absent result",
            })?;
        let eligible = self
            .eviction_index
            .eligible
            .remove(&(entry.last_access, node.clone()));
        entry.last_access = stamp;
        if eligible {
            self.eviction_index.eligible.insert((stamp, node.clone()));
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
                self.eviction_index
                    .eligible
                    .insert((entry.last_access, node.clone()));
            }
        }
        Ok(())
    }

    /// Called on every normal Result exit, including arithmetic, scheduling,
    /// admission and cache errors. Partially computed nodes are never cached.
    pub(super) fn release_request_pins(&mut self, pins: &mut DependencyPins<N>) {
        for (node, _) in std::mem::take(&mut pins.nodes) {
            if let Some(entry) = self.cache.get(&node) {
                self.eviction_index
                    .eligible
                    .insert((entry.last_access, node));
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
                        self.eviction_index
                            .eligible
                            .remove(&(previous.last_access, node.clone()));
                    }
                    let eligible = (evict && !pinned).then(|| node.clone());
                    self.cache.insert(
                        node,
                        CacheEntry {
                            terms,
                            conditions,
                            weight,
                            last_access: stamp,
                        },
                    );
                    self.cache_weight = prospective;
                    if let Some(node) = eligible {
                        self.eviction_index.eligible.insert((stamp, node));
                    }
                    return Ok(());
                }
                Err(error) if evict && is_cache_limit(&error) => {
                    let Some((access, victim)) = self.eviction_index.eligible.first().cloned()
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
                    self.eviction_index.eligible.remove(&(access, victim));
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
