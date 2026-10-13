//! Exact, memoized application of the same immutable routed owner programs.
//! No missing rule becomes a terminal and no source generation runs here.
mod engine;
mod transport;

use std::collections::BTreeMap;
use std::sync::Arc;

use super::super::cache::CachedTerms;
use super::super::{CandidateCacheRepresentation, CandidateStatistics};
use super::trace::Node;
use super::{CandidateRoutedError, RoutedCandidateReducer};
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::{CacheWeight, ReductionStatistics, SharedCacheBudget};

/// Per-call work. Cache census is available from the owning application's
/// `cache_statistics`; cache entries count operational Route/Apply states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedApplicationStatistics {
    pub input_terms: usize,
    pub requested_targets: usize,
    pub operational_nodes: usize,
    pub transport_calls: usize,
    pub transport_operations: usize,
    pub transport_endpoints: usize,
    pub work: ReductionStatistics,
}

/// Exact coefficients under sufficient generic nonzero conditions. Reduction
/// methods return declared terminals; `route_entry_to_owner` returns unreduced
/// owner images. This is not a source replay or family-closure proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateRoutedReduction {
    family_fingerprint: Arc<String>,
    terms: BTreeMap<IntegralKey, Coefficient>,
    nonzero_conditions: Vec<Coefficient>,
    statistics: CandidateRoutedApplicationStatistics,
}

impl CandidateRoutedReduction {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.terms
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.nonzero_conditions
    }
    pub fn statistics(&self) -> &CandidateRoutedApplicationStatistics {
        &self.statistics
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

#[derive(Debug)]
struct CacheEntry {
    terms: CachedTerms,
    conditions: Vec<Coefficient>,
    weight: CacheWeight,
}

/// Mutable application session over an immutable owner and route snapshot.
/// Entry admission is checked on every input before cancellation or cache
/// lookup. Descendants are constrained by owner rules, never by entry bounds.
#[derive(Debug)]
pub struct RoutedCandidateApplication<const N: usize> {
    reducer: RoutedCandidateReducer<N>,
    cache: BTreeMap<Node<N>, CacheEntry>,
    cache_representation: CandidateCacheRepresentation,
    cache_budget: SharedCacheBudget,
    cache_weight: CacheWeight,
    work: ReductionStatistics,
}

impl<const N: usize> RoutedCandidateReducer<N> {
    pub fn application(&self) -> RoutedCandidateApplication<N> {
        RoutedCandidateApplication {
            reducer: Self {
                programs: self.programs.clone(),
                routes: self.routes.clone(),
                limits: self.limits,
            },
            cache: BTreeMap::new(),
            cache_representation: CandidateCacheRepresentation::Sparse,
            cache_budget: SharedCacheBudget::default(),
            cache_weight: CacheWeight::default(),
            work: ReductionStatistics::default(),
        }
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    pub fn cache_representation(&self) -> CandidateCacheRepresentation {
        self.cache_representation
    }

    /// Select private coefficient storage before application or after clearing
    /// the cache. Factorized storage uses the existing native representation
    /// and conservative term/byte census; public outputs remain exact ordinary
    /// coefficients and are materialized only for requested roots.
    pub fn set_cache_representation(
        &mut self,
        representation: CandidateCacheRepresentation,
    ) -> Result<(), CandidateRoutedError> {
        if !self.cache.is_empty() {
            return Err(CandidateRoutedError::InvalidInput(
                "coefficient representation requires an empty routed reduction cache".into(),
            ));
        }
        self.cache_representation = representation;
        Ok(())
    }
    pub fn family(&self) -> &IntegralFamily {
        self.reducer.programs.context.family()
    }
    pub fn cache_statistics(&self) -> CandidateStatistics {
        let census = self.cache_budget.snapshot();
        CandidateStatistics {
            work: self.work,
            cached_integrals: census.integrals,
            cached_coefficient_terms: census.coefficient_terms,
            cached_coefficient_bytes: census.coefficient_bytes,
        }
    }
    pub fn clear_cache(&mut self) -> Result<(), CandidateRoutedError> {
        self.cache_budget.replace(
            self.cache.len(),
            self.cache_weight,
            0,
            CacheWeight::default(),
            self.reducer.programs.context.limits,
        )?;
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
