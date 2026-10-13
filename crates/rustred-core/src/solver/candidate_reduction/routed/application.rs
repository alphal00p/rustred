//! Exact, memoized application of the same immutable routed owner programs.
//! No missing rule becomes a terminal and no source generation runs here.
mod cache_policy;
mod engine;
mod guard_union;
mod transport;

use cache_policy::EvictionIndex;
pub use cache_policy::{CandidateRoutedCachePolicy, CandidateRoutedCacheStatistics};

use std::collections::BTreeMap;
use std::sync::Arc;

use super::super::cache::CachedTerms;
use super::super::owners::PreparedOwner;
use super::super::{
    CandidateCacheRepresentation, CandidateOwnerContext, CandidateOwnerPrograms,
    CandidateReductionError, CandidateStatistics, DeferredCandidateOwnerPrograms,
    DeferredCandidateOwnerStatistics,
};
use super::trace::Node;
use super::{
    CandidateOwnerRoute, CandidateRoutedError, RoutedCandidateLimits, RoutedCandidateReducer,
};
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
    pub cache: CandidateRoutedCacheStatistics,
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
    last_access: u64,
}

#[derive(Debug)]
enum ApplicationOwners<const N: usize> {
    Eager(Arc<CandidateOwnerPrograms<N>>),
    Deferred(Arc<DeferredCandidateOwnerPrograms<N>>),
}
impl<const N: usize> ApplicationOwners<N> {
    fn contains(&self, sector: &[bool; N]) -> bool {
        match self {
            Self::Eager(programs) => programs.owners.contains_key(sector),
            Self::Deferred(programs) => programs.contains_owner(sector),
        }
    }
    fn owner(
        &self,
        sector: &[bool; N],
    ) -> Result<Option<Arc<PreparedOwner<N>>>, CandidateReductionError> {
        match self {
            Self::Eager(programs) => Ok(programs.owners.get(sector).cloned()),
            Self::Deferred(programs) => programs.owner(sector),
        }
    }
}

/// Mutable application session over an immutable owner and route snapshot.
/// Entry admission is checked on every input before cancellation or cache
/// lookup. Descendants are constrained by owner rules, never by entry bounds.
#[derive(Debug)]
pub struct RoutedCandidateApplication<const N: usize> {
    context: Arc<CandidateOwnerContext<N>>,
    owners: ApplicationOwners<N>,
    routes: BTreeMap<[bool; N], CandidateOwnerRoute>,
    limits: RoutedCandidateLimits,
    cache: BTreeMap<Node<N>, CacheEntry>,
    cache_representation: CandidateCacheRepresentation,
    cache_policy: CandidateRoutedCachePolicy,
    eviction_index: EvictionIndex<N>,
    cache_work: CandidateRoutedCacheStatistics,
    cache_budget: SharedCacheBudget,
    cache_weight: CacheWeight,
    work: ReductionStatistics,
}

impl<const N: usize> RoutedCandidateReducer<N> {
    pub fn application(&self) -> RoutedCandidateApplication<N> {
        RoutedCandidateApplication {
            context: self.programs.context.clone(),
            owners: ApplicationOwners::Eager(self.programs.clone()),
            routes: self.routes.clone(),
            limits: self.limits,
            cache: BTreeMap::new(),
            cache_representation: CandidateCacheRepresentation::Sparse,
            cache_policy: CandidateRoutedCachePolicy::default(),
            eviction_index: EvictionIndex::default(),
            cache_work: CandidateRoutedCacheStatistics::default(),
            cache_budget: SharedCacheBudget::default(),
            cache_weight: CacheWeight::default(),
            work: ReductionStatistics::default(),
        }
    }
}

impl<const N: usize> RoutedCandidateApplication<N> {
    /// Construct a runtime application without preparing any owner formulas.
    /// Manifest presence authenticates route endpoints; first Apply performs
    /// that owner's native admission once and fails closed on any error.
    pub fn try_new_deferred(
        programs: Arc<DeferredCandidateOwnerPrograms<N>>,
        routes: impl IntoIterator<Item = CandidateOwnerRoute>,
        limits: RoutedCandidateLimits,
    ) -> Result<Self, CandidateRoutedError> {
        let routes = super::prepare::validate_routes(
            programs.context(),
            |sector| programs.contains_owner(sector),
            routes,
            limits,
        )?;
        Ok(Self {
            context: programs.context().clone(),
            owners: ApplicationOwners::Deferred(programs),
            routes,
            limits,
            cache: BTreeMap::new(),
            cache_representation: CandidateCacheRepresentation::Sparse,
            cache_policy: CandidateRoutedCachePolicy::default(),
            eviction_index: EvictionIndex::default(),
            cache_work: CandidateRoutedCacheStatistics::default(),
            cache_budget: SharedCacheBudget::default(),
            cache_weight: CacheWeight::default(),
            work: ReductionStatistics::default(),
        })
    }
    pub fn owner_preparation_statistics(&self) -> DeferredCandidateOwnerStatistics {
        match &self.owners {
            ApplicationOwners::Deferred(programs) => programs.statistics(),
            ApplicationOwners::Eager(programs) => DeferredCandidateOwnerStatistics {
                declared_owners: programs.owner_count(),
                prepared_owners: programs.owner_count(),
                ..Default::default()
            },
        }
    }
    pub fn cache_representation(&self) -> CandidateCacheRepresentation {
        self.cache_representation
    }

    pub fn cache_policy(&self) -> CandidateRoutedCachePolicy {
        self.cache_policy
    }

    /// Select bounded cache behavior before application or after clearing the
    /// cache. Eviction changes reuse only, never admission or terminal status.
    pub fn set_cache_policy(
        &mut self,
        policy: CandidateRoutedCachePolicy,
    ) -> Result<(), CandidateRoutedError> {
        if !self.cache.is_empty() {
            return Err(CandidateRoutedError::InvalidInput(
                "cache policy requires an empty routed reduction cache".into(),
            ));
        }
        self.cache_policy = policy;
        self.eviction_index = EvictionIndex::default();
        Ok(())
    }

    /// Cumulative work, including failed requests; clearing the cache does not
    /// reset work counters. Per-call work is also included in each result.
    pub fn cache_work_statistics(&self) -> CandidateRoutedCacheStatistics {
        self.cache_work
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
        self.context.family()
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
            self.context.limits,
        )?;
        self.cache.clear();
        self.cache_weight = CacheWeight::default();
        self.eviction_index = EvictionIndex::default();
        Ok(())
    }
}

#[cfg(test)]
mod tests;
