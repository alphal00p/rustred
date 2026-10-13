use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use super::super::{VacuumIntegralKey, VacuumProductError, VacuumProductLimits};
use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::{CacheWeight, ReductionError, ReductionLimits, SharedCacheBudget};

/// One unevaluated factor with its authenticated complete family definition.
#[derive(Clone, Debug)]
pub struct VacuumFactor {
    pub family: Arc<IntegralFamily>,
    pub integral: IntegralKey,
}

/// One declared terminal returned by the selected existing lower program.
#[derive(Clone, Debug)]
pub struct VacuumFactorTerm {
    pub family: Arc<IntegralFamily>,
    pub integral: IntegralKey,
    pub coefficient: Coefficient,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumFactorWork {
    pub rule_applications: usize,
    pub coalescing_additions: usize,
    pub cache_hits: usize,
}

#[derive(Clone, Debug, Default)]
pub struct VacuumFactorReduction {
    pub terms: Vec<VacuumFactorTerm>,
    pub nonzero_conditions: Vec<Coefficient>,
    pub work: VacuumFactorWork,
}

/// Immutable program selection with optional mutable preparation state.
///
/// The provider must apply already available programs under the supplied
/// remaining resource allowances, report per-call work, and return their exact
/// declared-terminal expansion and all original applicability conditions in the
/// same stored unit-mass loop-measure convention. It must error on a missing
/// program, never invent a terminal or generate IBPs. Returned families must
/// preserve loop count, dimension, and the exact coefficient map. Program
/// metadata may be retained; transient reduction caches obey the given limits.
/// Changing program identity requires a new `VacuumProductReducer` session.
pub trait VacuumFactorProvider {
    type Error: fmt::Display;
    fn reduce(
        &mut self,
        family: Arc<IntegralFamily>,
        target: &IntegralKey,
        limits: ReductionLimits,
    ) -> Result<VacuumFactorReduction, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VacuumProductReductionLimits {
    pub reduction: ReductionLimits,
    pub products: VacuumProductLimits,
    pub max_input_products: usize,
    pub max_factors_per_product: usize,
    pub max_provider_calls: usize,
    pub max_families: usize,
    pub max_nonzero_conditions: usize,
}
impl Default for VacuumProductReductionLimits {
    fn default() -> Self {
        Self {
            reduction: Default::default(),
            products: Default::default(),
            max_input_products: 100_000,
            max_factors_per_product: 1024,
            max_provider_calls: 100_000,
            max_families: 4096,
            max_nonzero_conditions: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumProductReductionStatistics {
    pub input_products: usize,
    pub provider_calls: usize,
    pub cache_hits: usize,
    pub provider_cache_hits: usize,
    pub rule_applications: usize,
    pub coalescing_additions: usize,
    pub product_terminals: usize,
    pub basis_candidates: usize,
    pub canonical_candidates: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VacuumProductCacheStatistics {
    pub cached_factors: usize,
    pub coefficient_terms: usize,
    pub coefficient_bytes: usize,
    pub families: usize,
}

pub(super) type Terms = BTreeMap<Vec<VacuumIntegralKey>, Coefficient>;

/// Sorted commutative products, with no assertion of master independence.
#[derive(Clone, Debug)]
pub struct VacuumProductReduction {
    pub(super) terms: Terms,
    pub(super) nonzero_conditions: Vec<Coefficient>,
    pub(super) families: BTreeMap<String, Arc<IntegralFamily>>,
    pub(super) statistics: VacuumProductReductionStatistics,
}
impl VacuumProductReduction {
    pub fn terms(&self) -> &BTreeMap<Vec<VacuumIntegralKey>, Coefficient> {
        &self.terms
    }
    pub fn nonzero_conditions(&self) -> &[Coefficient] {
        &self.nonzero_conditions
    }
    pub fn families(&self) -> &BTreeMap<String, Arc<IntegralFamily>> {
        &self.families
    }
    pub fn statistics(&self) -> &VacuumProductReductionStatistics {
        &self.statistics
    }
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

pub(super) struct CacheEntry {
    pub terms: Terms,
    pub conditions: Vec<Coefficient>,
}

/// One immutable provider identity and its exact qualified-factor cache.
pub struct VacuumProductReducer<P> {
    pub(super) provider: P,
    pub(super) anchor: Arc<IntegralFamily>,
    pub(super) context: CoefficientContext,
    pub(super) limits: VacuumProductReductionLimits,
    pub(super) families: BTreeMap<String, Arc<IntegralFamily>>,
    pub(super) cache: BTreeMap<VacuumIntegralKey, Arc<CacheEntry>>,
    pub(super) cache_weight: CacheWeight,
    pub(super) cache_budget: SharedCacheBudget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VacuumProductReductionError {
    InvalidInput(String),
    Provider {
        family: String,
        target: IntegralKey,
        message: String,
    },
    Reduction(ReductionError),
    Product(VacuumProductError),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}
impl From<ReductionError> for VacuumProductReductionError {
    fn from(error: ReductionError) -> Self {
        Self::Reduction(error)
    }
}
impl From<VacuumProductError> for VacuumProductReductionError {
    fn from(error: VacuumProductError) -> Self {
        Self::Product(error)
    }
}
impl From<crate::algebra::ExactAlgebraError> for VacuumProductReductionError {
    fn from(error: crate::algebra::ExactAlgebraError) -> Self {
        Self::Reduction(error.into())
    }
}
impl fmt::Display for VacuumProductReductionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid product reduction: {message}"),
            Self::Provider {
                target, message, ..
            } => write!(
                f,
                "factor provider failed at {:?}: {message}",
                target.powers()
            ),
            Self::Reduction(error) => error.fmt(f),
            Self::Product(error) => error.fmt(f),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "product reduction {resource} requires {requested}; limit {limit}"
            ),
        }
    }
}
impl std::error::Error for VacuumProductReductionError {}
