//! Exact positive-power vacuum products, with symbolic lower-family factors.
//! Momentum squares, inverse matrices and full block separation are replayed;
//! no IBPs, numerical values, terminal independence or closure are inferred.
mod canonical;
mod charts;
mod prepare;

pub use charts::{
    VacuumProductChartLimits, VacuumProductChartPlan, VacuumProductComponent,
    VacuumProductProjection, VacuumTensorProjection, VacuumTensorProjectionInput,
    VerifiedVacuumProductChart,
};

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use super::VacuumIntegralKey;
use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VacuumProductLimits {
    pub diagnostics_enabled: bool,
    pub max_terminals: usize,
    pub max_lower_families: usize,
    /// Shared over structural basis searches and all lower-factor charts.
    pub max_basis_candidates: usize,
    /// Shared over all signed coordinate permutations during preparation.
    pub max_canonical_candidates: usize,
}
impl Default for VacuumProductLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_terminals: 100_000,
            max_lower_families: 1024,
            max_basis_candidates: 100_000,
            max_canonical_candidates: 2_000_000,
        }
    }
}

impl VacuumProductLimits {
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
    pub(crate) const fn diagnostic_limit(self, limit: usize) -> Option<usize> {
        if self.diagnostics_enabled {
            Some(limit)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VacuumProductStatistics {
    pub requested_terminals: usize,
    pub factored_terminals: usize,
    /// Connected or unsupported terms remain explicit original-family keys.
    pub retained_terminals: usize,
    pub numerator_terminals: usize,
    pub analyzed_supports: usize,
    pub basis_candidates: usize,
    pub canonical_candidates: usize,
}

/// A verified equality in the stored unit-mass loop-measure convention.
/// Repeated factors remain repeated entries in this sorted commutative product.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedVacuumProduct {
    coefficient: Coefficient,
    factors: Vec<VacuumIntegralKey>,
}
impl VerifiedVacuumProduct {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
    pub fn factors(&self) -> &[VacuumIntegralKey] {
        &self.factors
    }
}

/// Prepared once per finite declared terminal set. Missing entries are left
/// unchanged: unsupported numerator couplings are never called products.
#[derive(Debug)]
pub struct VacuumProductPlan {
    family_fingerprint: Arc<String>,
    products: BTreeMap<IntegralKey, VerifiedVacuumProduct>,
    families: BTreeMap<String, Arc<IntegralFamily>>,
    statistics: VacuumProductStatistics,
}
impl VacuumProductPlan {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }
    pub fn products(&self) -> &BTreeMap<IntegralKey, VerifiedVacuumProduct> {
        &self.products
    }
    pub fn families(&self) -> &BTreeMap<String, Arc<IntegralFamily>> {
        &self.families
    }
    pub fn statistics(&self) -> &VacuumProductStatistics {
        &self.statistics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VacuumProductError {
    Resources(symbolica::utils::resources::ResourceLimitError),
    InvalidInput(String),
    ExactAlgebra(String),
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}
impl fmt::Display for VacuumProductError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resources(error) => error.fmt(f),
            Self::InvalidInput(message) => write!(f, "invalid vacuum product: {message}"),
            Self::ExactAlgebra(message) => write!(f, "vacuum product exact algebra: {message}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "vacuum product {resource} requires {requested}; limit {limit}"
            ),
        }
    }
}
impl std::error::Error for VacuumProductError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resources(error) => Some(error),
            _ => None,
        }
    }
}

fn algebra(error: impl std::error::Error + 'static) -> VacuumProductError {
    match crate::reduction::resource_limit_error(&error) {
        Some(resource) => VacuumProductError::Resources(resource.clone()),
        None => VacuumProductError::ExactAlgebra(error.to_string()),
    }
}
fn charge(
    count: &mut usize,
    limit: Option<usize>,
    resource: &'static str,
) -> Result<(), VacuumProductError> {
    symbolica::utils::resources::checkpoint().map_err(VacuumProductError::Resources)?;
    let requested = count.saturating_add(1);
    if let Some(limit) = limit {
        if requested > limit {
            return Err(VacuumProductError::ResourceLimit {
                resource,
                requested,
                limit,
            });
        }
    }
    *count = requested;
    Ok(())
}

#[cfg(test)]
mod tests;
