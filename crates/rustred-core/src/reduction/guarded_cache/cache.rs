use super::{ClosedUnitMassProgram, GuardedCacheError};
use crate::algebra::ExactAlgebraLimits;
use crate::family::IntegralKey;
use crate::persistence::application_cache::{ApplicationCacheBinding, ApplicationCacheRecord};
use crate::reduction::cache_policy::{
    EvictionIndex, EvictionMetadata, check_entry, is_cache_limit,
};
use crate::reduction::{
    CacheWeight, CompletedCachePolicy, ReductionError, ReductionLimits, ReductionStatistics,
    SharedCacheBudget, coefficient_cache_weight,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

struct Entry {
    record: ApplicationCacheRecord,
    weight: CacheWeight,
    eviction: EvictionMetadata,
    validated_exact: ExactAlgebraLimits,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GuardedRootCacheStatistics {
    pub ram_hits: usize,
    pub backing_hits: usize,
    pub completed_reductions: usize,
    /// Full payload-validation attempts on admission or tightened exact
    /// bounds. Unchanged/looser RAM hits do not traverse the payload again.
    pub payload_validations: usize,
    pub evicted_entries: usize,
    pub evicted_coefficient_terms: usize,
    pub evicted_coefficient_bytes: usize,
    pub cached_integrals: usize,
    pub cached_coefficient_terms: usize,
    pub cached_coefficient_bytes: usize,
    /// Successful native reduction work; a refused native call returns no
    /// partial work receipt and is not invented here.
    pub work: ReductionStatistics,
}

struct State {
    policy: CompletedCachePolicy,
    entries: BTreeMap<IntegralKey, Entry>,
    weight: CacheWeight,
    index: EvictionIndex<IntegralKey>,
    statistics: GuardedRootCacheStatistics,
}

impl State {
    fn evict_one(
        &mut self,
        budget: &SharedCacheBudget,
        limits: ReductionLimits,
    ) -> Result<bool, ReductionError> {
        if !self.policy.evicts() {
            return Ok(false);
        }
        let Some((priority, access, victim)) = self.index.eligible.first().cloned() else {
            return Ok(false);
        };
        let entry = self
            .entries
            .get(&victim)
            .ok_or(ReductionError::ReducerInvariant {
                detail: "guarded root eviction points to absent entry",
            })?;
        let weight = entry.weight;
        let remaining = self.weight.checked_sub(weight)?;
        // Never re-enter the pressure callback while the root state is locked.
        budget.replace_without_pressure(1, weight, 0, CacheWeight::default(), limits)?;
        self.entries.remove(&victim);
        self.index.evicted(priority, access, victim);
        self.weight = remaining;
        self.statistics.evicted_entries = self.statistics.evicted_entries.saturating_add(1);
        self.statistics.evicted_coefficient_terms = self
            .statistics
            .evicted_coefficient_terms
            .saturating_add(weight.coefficient_terms);
        self.statistics.evicted_coefficient_bytes = self
            .statistics
            .evicted_coefficient_bytes
            .saturating_add(weight.coefficient_bytes);
        Ok(true)
    }
}

/// Caller-owned mutable cache, bound to one immutable program. Coefficient
/// payload accounting includes guards, including guarded zero results. Map
/// metadata is bounded by integral count; this is not a process-RSS bound.
/// A cold solve shares the same budget and can evict completed outer roots;
/// its active descendants remain protected inside the ordinary reducer.
pub struct GuardedRootCache {
    pub(super) binding: ApplicationCacheBinding,
    pub(super) artifact_identity: usize,
    pub(super) limits: ReductionLimits,
    state: Arc<Mutex<State>>,
    budget: Arc<SharedCacheBudget>,
}

impl GuardedRootCache {
    pub fn new(program: &ClosedUnitMassProgram<'_>, limits: ReductionLimits) -> Self {
        Self::with_policy(
            program,
            limits,
            CompletedCachePolicy::EvictLeastRecentlyUsed,
        )
    }
    pub fn with_policy(
        program: &ClosedUnitMassProgram<'_>,
        limits: ReductionLimits,
        policy: CompletedCachePolicy,
    ) -> Self {
        let state = Arc::new(Mutex::new(State {
            policy,
            entries: BTreeMap::new(),
            weight: CacheWeight::default(),
            index: EvictionIndex::default(),
            statistics: GuardedRootCacheStatistics::default(),
        }));
        let budget = Arc::new(SharedCacheBudget::default());
        if policy.evicts() {
            let weak_state = Arc::downgrade(&state);
            let weak_budget = Arc::downgrade(&budget);
            budget.set_pressure_handler(Arc::new(move || {
                let (Some(state), Some(budget)) = (weak_state.upgrade(), weak_budget.upgrade())
                else {
                    return Ok(false);
                };
                let mut state = state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state.evict_one(&budget, limits)
            }));
        }
        Self {
            binding: program.binding().clone(),
            artifact_identity: program.artifact_identity(),
            limits,
            state,
            budget,
        }
    }
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub fn binding(&self) -> &ApplicationCacheBinding {
        &self.binding
    }
    pub fn policy(&self) -> CompletedCachePolicy {
        self.state().policy
    }
    pub fn limits(&self) -> ReductionLimits {
        self.limits
    }
    pub fn statistics(&self) -> GuardedRootCacheStatistics {
        let state = self.state();
        let mut statistics = state.statistics;
        statistics.cached_integrals = state.entries.len();
        statistics.cached_coefficient_terms = state.weight.coefficient_terms;
        statistics.cached_coefficient_bytes = state.weight.coefficient_bytes;
        statistics
    }
    /// Clear only RAM payload and operational eviction metadata, not backing
    /// files or cumulative work counters. No seeded descendants are retained.
    pub fn clear(&mut self) -> Result<(), ReductionError> {
        let mut state = self.state();
        self.budget.replace_without_pressure(
            state.entries.len(),
            state.weight,
            0,
            CacheWeight::default(),
            self.limits,
        )?;
        state.entries.clear();
        state.weight = CacheWeight::default();
        state.index = EvictionIndex::default();
        Ok(())
    }
    pub(super) fn check_limits(&self, limits: ReductionLimits) -> Result<(), ReductionError> {
        self.budget
            .replace(0, CacheWeight::default(), 0, CacheWeight::default(), limits)
    }
    pub(super) fn shared_budget(&self) -> Arc<SharedCacheBudget> {
        Arc::clone(&self.budget)
    }
    pub(super) fn check_hit_weight(
        &self,
        key: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<(), ReductionError> {
        if let Some(entry) = self.state().entries.get(key) {
            check_entry(entry.weight, limits)?;
        }
        Ok(())
    }
    #[cfg(test)]
    pub(super) fn peek(&self, key: &IntegralKey) -> Option<ApplicationCacheRecord> {
        self.state()
            .entries
            .get(key)
            .map(|entry| entry.record.clone())
    }
    pub(super) fn hit(
        &mut self,
        key: &IntegralKey,
        limits: ReductionLimits,
        validate: impl FnOnce(&ApplicationCacheRecord) -> Result<(), GuardedCacheError>,
    ) -> Result<Option<ApplicationCacheRecord>, GuardedCacheError> {
        let mut state = self.state();
        let Some(entry) = state.entries.get(key) else {
            return Ok(None);
        };
        check_entry(entry.weight, limits)?;
        let validated = entry.validated_exact;
        let current = limits.exact_algebra;
        let covered = current.max_exponent >= validated.max_exponent
            && current.max_polynomial_terms >= validated.max_polynomial_terms
            && current.max_term_operations >= validated.max_term_operations;
        if !covered {
            state.statistics.payload_validations =
                state.statistics.payload_validations.saturating_add(1);
            validate(&state.entries[key].record)?;
            state
                .entries
                .get_mut(key)
                .expect("checked present")
                .validated_exact = current;
        }
        let previous = state.entries[key].eviction;
        if state.policy.evicts() {
            let policy = state.policy;
            let metadata = state.index.accessed(policy, previous, true)?;
            state.index.remove(key, previous);
            state
                .entries
                .get_mut(key)
                .expect("checked present")
                .eviction = metadata;
            state.index.insert(key.clone(), metadata);
        }
        state.statistics.ram_hits = state.statistics.ram_hits.saturating_add(1);
        Ok(Some(state.entries[key].record.clone()))
    }
    pub(super) fn validate_payload(
        &mut self,
        validate: impl FnOnce() -> Result<(), GuardedCacheError>,
    ) -> Result<(), GuardedCacheError> {
        {
            let mut state = self.state();
            state.statistics.payload_validations =
                state.statistics.payload_validations.saturating_add(1);
        }
        validate()
    }
    pub(super) fn backing_hit(&mut self) {
        let mut state = self.state();
        state.statistics.backing_hits = state.statistics.backing_hits.saturating_add(1);
    }
    pub(super) fn completed_reduction(&mut self, work: ReductionStatistics) {
        let mut state = self.state();
        state.statistics.completed_reductions =
            state.statistics.completed_reductions.saturating_add(1);
        state.statistics.work.merge_work(work);
    }
    pub(super) fn insert(
        &mut self,
        key: IntegralKey,
        record: ApplicationCacheRecord,
        limits: ReductionLimits,
    ) -> Result<(), ReductionError> {
        let weight =
            coefficient_cache_weight(record.terms.values().chain(&record.nonzero_conditions))?;
        check_entry(weight, limits)?;
        let mut state = self.state();
        let stamp = if state.policy.evicts() {
            state.index.next_access()?
        } else {
            0
        };
        loop {
            let eviction = state.index.admission(state.policy, stamp)?;
            let previous = state.entries.get(&key);
            let previous_weight = previous.map(|entry| entry.weight).unwrap_or_default();
            let prospective = state
                .weight
                .checked_sub(previous_weight)?
                .checked_add(weight)?;
            match self.budget.replace_without_pressure(
                usize::from(previous.is_some()),
                previous_weight,
                1,
                weight,
                limits,
            ) {
                Ok(()) => {
                    let previous = previous.map(|entry| entry.eviction);
                    if let Some(previous) = previous {
                        state.index.remove(&key, previous);
                    }
                    state.entries.insert(
                        key.clone(),
                        Entry {
                            record,
                            weight,
                            eviction,
                            validated_exact: limits.exact_algebra,
                        },
                    );
                    state.weight = prospective;
                    if state.policy.evicts() {
                        state.index.insert(key, eviction);
                    }
                    return Ok(());
                }
                Err(error) if state.policy.evicts() && is_cache_limit(&error) => {
                    if !state.evict_one(&self.budget, limits)? {
                        return Err(error);
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
}
