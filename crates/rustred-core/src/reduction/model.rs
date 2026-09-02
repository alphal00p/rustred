use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::{Coefficient, ExactAlgebraLimits, IndexedAlgebraLimits};
use crate::family::IntegralKey;

/// Hot-path resource policy for applying an already sealed artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReductionLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub indexed_algebra: IndexedAlgebraLimits,
    pub max_rule_applications: usize,
    pub max_cached_integrals: usize,
    /// Aggregate numerator-plus-denominator sparse terms retained by every
    /// coefficient in the memoization cache.
    pub max_cached_coefficient_terms: usize,
    /// Aggregate clone-owned Symbolica coefficient payload retained by the
    /// memoization cache. This excludes map/key bookkeeping, whose counts are
    /// bounded independently by the integral and coefficient-term limits.
    pub max_cached_coefficient_bytes: usize,
    pub max_pending_frames: usize,
    /// Maximum coefficient additions that merge like terms during one
    /// top-level reduction request, including nested factorized recurrences.
    pub max_coalescing_additions: usize,
    /// Maximum exact Cartesian terms admitted at any intermediate step of a
    /// lower-family product factorization.
    pub max_factorization_terms: usize,
}

impl Default for ReductionLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            indexed_algebra: IndexedAlgebraLimits::default(),
            max_rule_applications: 1_000_000,
            max_cached_integrals: 1_000_000,
            max_cached_coefficient_terms: 16_000_000,
            max_cached_coefficient_bytes: 1024 * 1024 * 1024,
            max_pending_frames: 1_000_000,
            max_coalescing_additions: 16_000_000,
            max_factorization_terms: 1_000_000,
        }
    }
}

/// Observable work counters for one memoizing reducer owner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReductionStatistics {
    cache_hits: usize,
    rule_applications: usize,
    coalescing_additions: usize,
    cached_integrals: usize,
    cached_coefficient_terms: usize,
    cached_coefficient_bytes: usize,
}

impl ReductionStatistics {
    pub fn cache_hits(self) -> usize {
        self.cache_hits
    }

    pub fn rule_applications(self) -> usize {
        self.rule_applications
    }

    pub fn coalescing_additions(self) -> usize {
        self.coalescing_additions
    }

    pub fn cached_integrals(self) -> usize {
        self.cached_integrals
    }

    pub fn cached_coefficient_terms(self) -> usize {
        self.cached_coefficient_terms
    }

    pub fn cached_coefficient_bytes(self) -> usize {
        self.cached_coefficient_bytes
    }

    pub(crate) fn record_cache_hit(&mut self) {
        self.cache_hits = self.cache_hits.saturating_add(1);
    }

    pub(crate) fn record_rule_application(&mut self) {
        self.record_rule_applications(1);
    }

    pub(crate) fn record_rule_applications(&mut self, count: usize) {
        self.rule_applications = self.rule_applications.saturating_add(count);
    }

    pub(crate) fn record_coalescing_additions(&mut self, count: usize) {
        self.coalescing_additions = self.coalescing_additions.saturating_add(count);
    }

    pub(crate) fn merge_work(&mut self, other: Self) {
        self.cache_hits = self.cache_hits.saturating_add(other.cache_hits);
        self.rule_applications = self
            .rule_applications
            .saturating_add(other.rule_applications);
        self.coalescing_additions = self
            .coalescing_additions
            .saturating_add(other.coalescing_additions);
    }

    pub(super) fn set_cache_census(
        &mut self,
        cached_integrals: usize,
        cached_coefficient_terms: usize,
        cached_coefficient_bytes: usize,
    ) {
        self.cached_integrals = cached_integrals;
        self.cached_coefficient_terms = cached_coefficient_terms;
        self.cached_coefficient_bytes = cached_coefficient_bytes;
    }
}

/// Exact coefficients of typed master integral keys.
///
/// A `BTreeMap` gives deterministic iteration and makes like-master
/// collection part of the value representation rather than a presentation
/// convention.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MasterDecomposition {
    family_fingerprint: Arc<String>,
    target: IntegralKey,
    terms: BTreeMap<IntegralKey, Coefficient>,
}

/// One exact unit-mass coefficient together with the common-scale monomial
/// needed to restore dimensions in a caller-owned coefficient context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomogeneousMasterCoefficient {
    unit_mass_coefficient: Coefficient,
    common_mass_squared_power: i128,
}

impl HomogeneousMasterCoefficient {
    pub fn unit_mass_coefficient(&self) -> &Coefficient {
        &self.unit_mass_coefficient
    }

    pub fn common_mass_squared_power(&self) -> i128 {
        self.common_mass_squared_power
    }

    pub(super) fn new(unit_mass_coefficient: Coefficient, common_mass_squared_power: i128) -> Self {
        Self {
            unit_mass_coefficient,
            common_mass_squared_power,
        }
    }
}

/// Deterministic master map with common-mass dependence restored as explicit
/// dimensional-homogeneity powers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HomogeneousMasterDecomposition {
    family_fingerprint: Arc<String>,
    target: IntegralKey,
    terms: BTreeMap<IntegralKey, HomogeneousMasterCoefficient>,
}

impl HomogeneousMasterDecomposition {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn target(&self) -> &IntegralKey {
        &self.target
    }

    pub fn terms(&self) -> &BTreeMap<IntegralKey, HomogeneousMasterCoefficient> {
        &self.terms
    }

    pub fn coefficient(&self, master: &IntegralKey) -> Option<&HomogeneousMasterCoefficient> {
        self.terms.get(master)
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub(super) fn new(
        family_fingerprint: Arc<String>,
        target: IntegralKey,
        terms: BTreeMap<IntegralKey, HomogeneousMasterCoefficient>,
    ) -> Self {
        Self {
            family_fingerprint,
            target,
            terms,
        }
    }

    pub(super) fn into_terms(self) -> BTreeMap<IntegralKey, HomogeneousMasterCoefficient> {
        self.terms
    }
}

impl MasterDecomposition {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn target(&self) -> &IntegralKey {
        &self.target
    }

    pub fn terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.terms
    }

    pub fn coefficient(&self, master: &IntegralKey) -> Option<&Coefficient> {
        self.terms.get(master)
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub(super) fn new(
        family_fingerprint: Arc<String>,
        target: IntegralKey,
        terms: BTreeMap<IntegralKey, Coefficient>,
    ) -> Self {
        Self {
            family_fingerprint,
            target,
            terms,
        }
    }

    pub(super) fn into_terms(self) -> BTreeMap<IntegralKey, Coefficient> {
        self.terms
    }
}
