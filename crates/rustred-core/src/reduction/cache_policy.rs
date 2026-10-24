//! Shared bounded eviction metadata for completed exact results. Callers own
//! payloads, budgets and eligibility (including any dependency pins).
use std::collections::BTreeSet;

use super::{CacheWeight, ReductionError, ReductionLimits};

/// Operational policy only; never changes arithmetic, guards or admission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompletedCachePolicy {
    /// Keep completed entries and refuse an insertion that exceeds a cap.
    #[default]
    FailOnLimit,
    /// Evict the least-recently-used eligible completed result.
    EvictLeastRecentlyUsed,
    /// Exact per-residency reuse counts with dynamic aging and recency ties.
    /// Only genuine completed-result reuse increments frequency. Admission
    /// priority is age + 1; eviction advances age to the victim's priority.
    EvictAgedLeastFrequentlyUsed,
}

impl CompletedCachePolicy {
    pub(crate) fn evicts(self) -> bool {
        self != Self::FailOnLimit
    }
}

/// Operational metadata only: never serialized as mathematical authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct EvictionMetadata {
    pub(crate) last_access: u64,
    pub(crate) reuse_count: u64,
    pub(crate) priority: u64,
}

#[derive(Debug)]
pub(crate) struct EvictionIndex<K> {
    // One record per eligible entry, no stale priority records or aging scan.
    pub(crate) eligible: BTreeSet<(u64, u64, K)>,
    pub(crate) clock: u64,
    pub(crate) age: u64,
}

impl<K> Default for EvictionIndex<K> {
    fn default() -> Self {
        Self {
            eligible: BTreeSet::new(),
            clock: 0,
            age: 0,
        }
    }
}

impl<K> EvictionIndex<K> {
    pub(crate) fn next_access(&mut self) -> Result<u64, ReductionError> {
        self.clock =
            self.clock
                .checked_add(1)
                .ok_or(ReductionError::CacheResourceCountOverflow {
                    resource: "completed cache access sequence",
                })?;
        Ok(self.clock)
    }

    pub(crate) fn aged_priority(&self, reuse_count: u64) -> Result<u64, ReductionError> {
        self.age
            .checked_add(1)
            .and_then(|age| age.checked_add(reuse_count))
            .ok_or(ReductionError::CacheResourceCountOverflow {
                resource: "completed LFU aging priority",
            })
    }

    pub(crate) fn admission(
        &self,
        policy: CompletedCachePolicy,
        stamp: u64,
    ) -> Result<EvictionMetadata, ReductionError> {
        Ok(EvictionMetadata {
            last_access: stamp,
            reuse_count: 0,
            priority: if policy == CompletedCachePolicy::EvictAgedLeastFrequentlyUsed {
                self.aged_priority(0)?
            } else {
                0
            },
        })
    }

    /// Compute all checked updates before the caller changes payload/index.
    pub(crate) fn accessed(
        &mut self,
        policy: CompletedCachePolicy,
        mut metadata: EvictionMetadata,
        reuse: bool,
    ) -> Result<EvictionMetadata, ReductionError> {
        metadata.last_access = self.next_access()?;
        if reuse && policy == CompletedCachePolicy::EvictAgedLeastFrequentlyUsed {
            metadata.reuse_count = metadata.reuse_count.checked_add(1).ok_or(
                ReductionError::CacheResourceCountOverflow {
                    resource: "completed LFU reuse frequency",
                },
            )?;
            metadata.priority = self.aged_priority(metadata.reuse_count)?;
        }
        Ok(metadata)
    }
}

impl<K: Ord + Clone> EvictionIndex<K> {
    pub(crate) fn remove(&mut self, key: &K, metadata: EvictionMetadata) -> bool {
        self.eligible
            .remove(&(metadata.priority, metadata.last_access, key.clone()))
    }
    pub(crate) fn insert(&mut self, key: K, metadata: EvictionMetadata) {
        self.eligible
            .insert((metadata.priority, metadata.last_access, key));
    }
    pub(crate) fn evicted(&mut self, priority: u64, access: u64, key: K) {
        self.eligible.remove(&(priority, access, key));
        // Returning old pinned entries must not rewind the aging baseline.
        self.age = self.age.max(priority);
    }
}

pub(crate) fn check_entry(
    weight: CacheWeight,
    limits: ReductionLimits,
) -> Result<(), ReductionError> {
    if limits.diagnostics_enabled && limits.max_cached_integrals == 0 {
        return Err(ReductionError::CacheLimit {
            requested: 1,
            limit: 0,
        });
    }
    if limits.diagnostics_enabled && weight.coefficient_terms > limits.max_cached_coefficient_terms
    {
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

pub(crate) fn is_cache_limit(error: &ReductionError) -> bool {
    matches!(
        error,
        ReductionError::CacheLimit { .. }
            | ReductionError::CacheCoefficientTermLimit { .. }
            | ReductionError::CacheCoefficientByteLimit { .. }
    )
}
