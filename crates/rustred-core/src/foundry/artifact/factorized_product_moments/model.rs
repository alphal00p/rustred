//! Immutable values for one non-owning product-moment computation.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralKey;

use super::super::ClosedTerminalAuthority;
use super::super::factorized_numerator_lift::FactorizedNumeratorLiftCompilation;
use super::error::FactorizedProductMomentError;

#[derive(Debug)]
pub(super) struct SingletonProductBlock {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_position: usize,
    pub(super) transformed_vector: usize,
    pub(super) active_power_ordinal: usize,
}

#[derive(Debug)]
pub(super) struct CorrelatedProductBlock {
    pub(super) dependency_ordinal: usize,
    pub(super) parent_positions: Box<[usize]>,
    pub(super) transformed_vectors: Box<[usize]>,
    /// Signs relating the routed vectors to the installed dependency vectors:
    /// `r_i = sign_i * t_i`.
    pub(super) vector_signs: Box<[i64]>,
    pub(super) active_power_start: usize,
}

#[derive(Debug)]
pub(super) enum ProductBlockLayout {
    AllSingleton {
        singletons_by_vector: Box<[SingletonProductBlock]>,
    },
    OneCorrelated {
        correlated: CorrelatedProductBlock,
        singletons_by_vector: Box<[SingletonProductBlock]>,
    },
}

/// Cold chart compiled from one admitted installed product factorization rule.
///
/// The exact routing compilation is retained as its provenance capability.
/// This type intentionally has no artifact/persistence conversion.
#[derive(Debug)]
pub(crate) struct FactorizedProductMomentChart<'authority> {
    pub(super) authority: &'authority ClosedTerminalAuthority,
    pub(super) factorization_ordinal: usize,
    pub(super) identity: Arc<()>,
    pub(super) routing: FactorizedNumeratorLiftCompilation,
    pub(super) layout: ProductBlockLayout,
    pub(super) active_parent_positions: Box<[usize]>,
    pub(super) edges: Box<[(usize, usize)]>,
    pub(super) radial_coordinate_positions: Box<[usize]>,
    pub(super) cross_coordinate_positions: Box<[usize]>,
    pub(super) normalization: Coefficient,
    pub(super) sole_raw_master: Option<IntegralKey>,
    pub(super) sole_terminal: Option<IntegralKey>,
}

impl FactorizedProductMomentChart<'_> {
    pub(crate) fn loop_factor_count(&self) -> usize {
        self.authority.family().loop_count()
    }

    pub(crate) fn cross_coordinate_count(&self) -> usize {
        self.edges.len()
    }

    pub(crate) fn singleton_factor_count(&self) -> usize {
        match &self.layout {
            ProductBlockLayout::AllSingleton {
                singletons_by_vector,
            }
            | ProductBlockLayout::OneCorrelated {
                singletons_by_vector,
                ..
            } => singletons_by_vector.len(),
        }
    }

    pub(crate) fn correlated_factor_loop_count(&self) -> Option<usize> {
        match &self.layout {
            ProductBlockLayout::AllSingleton { .. } => None,
            ProductBlockLayout::OneCorrelated { correlated, .. } => {
                Some(correlated.transformed_vectors.len())
            }
        }
    }

    pub(crate) fn terminal(&self) -> &IntegralKey {
        self.sole_terminal
            .as_ref()
            .expect("the all-singleton chart has one installed terminal")
    }

    pub(crate) fn normalization(&self) -> &Coefficient {
        &self.normalization
    }

    pub(crate) fn signed_loop_basis(&self) -> &[i64] {
        self.routing.routing().signed_loop_basis()
    }
    pub(super) fn rule(&self) -> &super::super::FactorizationRule {
        &self.authority.factorization_rules()[self.factorization_ordinal]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductMomentMonomial {
    active_powers: Box<[i64]>,
    radial_powers: Box<[u64]>,
    cross_powers: Box<[u64]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ProductMomentSource {
    Parent(IntegralKey),
    Monomial(ProductMomentMonomial),
}

impl ProductMomentMonomial {
    pub(crate) fn try_new<A, R, C>(
        active_powers: A,
        radial_powers: R,
        cross_powers: C,
    ) -> Result<Self, FactorizedProductMomentError>
    where
        A: IntoIterator<Item = i64>,
        A::IntoIter: ExactSizeIterator,
        R: IntoIterator<Item = u64>,
        R::IntoIter: ExactSizeIterator,
        C: IntoIterator<Item = u64>,
        C::IntoIter: ExactSizeIterator,
    {
        let active_powers = collect_fallibly(active_powers, "product active powers")?;
        let radial_powers = collect_fallibly(radial_powers, "product radial powers")?;
        let cross_powers = collect_fallibly(cross_powers, "product cross powers")?;
        Ok(Self {
            active_powers,
            radial_powers,
            cross_powers,
        })
    }

    pub(super) fn active_powers(&self) -> &[i64] {
        &self.active_powers
    }

    pub(super) fn radial_powers(&self) -> &[u64] {
        &self.radial_powers
    }

    pub(super) fn cross_powers(&self) -> &[u64] {
        &self.cross_powers
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProductMomentGuard {
    vector: usize,
    rank: u64,
    nonzero_polynomial: Coefficient,
}

impl ProductMomentGuard {
    pub(super) fn new(vector: usize, rank: u64, nonzero_polynomial: Coefficient) -> Self {
        Self {
            vector,
            rank,
            nonzero_polynomial,
        }
    }

    pub(crate) fn vector(&self) -> usize {
        self.vector
    }

    pub(crate) fn rank(&self) -> u64 {
        self.rank
    }

    pub(crate) fn nonzero_polynomial(&self) -> &Coefficient {
        &self.nonzero_polynomial
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProductMomentStatistics {
    pub(super) numerator_polynomial_terms: usize,
    pub(super) angular_states: usize,
    pub(super) angular_transitions: usize,
    pub(super) radial_states: usize,
    pub(super) radial_summands: usize,
    pub(super) dependency_requests: usize,
    pub(super) dependency_rule_applications: usize,
    pub(super) dependency_cache_hits: usize,
    pub(super) coalescing_additions: usize,
}

impl ProductMomentStatistics {
    pub(crate) fn numerator_polynomial_terms(self) -> usize {
        self.numerator_polynomial_terms
    }

    pub(crate) fn angular_states(self) -> usize {
        self.angular_states
    }

    pub(crate) fn angular_transitions(self) -> usize {
        self.angular_transitions
    }

    pub(crate) fn radial_states(self) -> usize {
        self.radial_states
    }

    pub(crate) fn radial_summands(self) -> usize {
        self.radial_summands
    }

    pub(crate) fn dependency_requests(self) -> usize {
        self.dependency_requests
    }

    pub(crate) fn dependency_rule_applications(self) -> usize {
        self.dependency_rule_applications
    }

    pub(crate) fn dependency_cache_hits(self) -> usize {
        self.dependency_cache_hits
    }

    pub(crate) fn coalescing_additions(self) -> usize {
        self.coalescing_additions
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProductMomentExpansion {
    family_fingerprint: Arc<String>,
    chart_identity: Arc<()>,
    source: ProductMomentSource,
    terms: BTreeMap<IntegralKey, Coefficient>,
    guards: Box<[ProductMomentGuard]>,
    statistics: ProductMomentStatistics,
}

impl ProductMomentExpansion {
    pub(super) fn new(
        family_fingerprint: Arc<String>,
        chart_identity: Arc<()>,
        source: ProductMomentSource,
        terms: BTreeMap<IntegralKey, Coefficient>,
        guards: Box<[ProductMomentGuard]>,
        statistics: ProductMomentStatistics,
    ) -> Self {
        Self {
            family_fingerprint,
            chart_identity,
            source,
            terms,
            guards,
            statistics,
        }
    }

    pub(crate) fn terms(&self) -> &BTreeMap<IntegralKey, Coefficient> {
        &self.terms
    }

    pub(crate) fn guards(&self) -> &[ProductMomentGuard] {
        &self.guards
    }

    pub(crate) fn statistics(&self) -> ProductMomentStatistics {
        self.statistics
    }

    pub(crate) fn belongs_to_chart(&self, chart: &FactorizedProductMomentChart<'_>) -> bool {
        Arc::ptr_eq(&self.chart_identity, &chart.identity)
            && Arc::ptr_eq(
                &self.family_fingerprint,
                &chart.authority.family().fingerprint_owner(),
            )
    }
}

impl PartialEq for ProductMomentExpansion {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.chart_identity, &other.chart_identity)
            && Arc::ptr_eq(&self.family_fingerprint, &other.family_fingerprint)
            && self.source == other.source
            && self.terms == other.terms
            && self.guards == other.guards
            && self.statistics == other.statistics
    }
}

impl Eq for ProductMomentExpansion {}

fn collect_fallibly<T, I>(
    values: I,
    resource: &'static str,
) -> Result<Box<[T]>, FactorizedProductMomentError>
where
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
{
    let iterator = values.into_iter();
    let mut output = Vec::new();
    let requested = iterator.len();
    output.try_reserve_exact(requested).map_err(|_| {
        FactorizedProductMomentError::AllocationFailure {
            resource,
            requested,
        }
    })?;
    for value in iterator {
        output.push(value);
    }
    Ok(output.into_boxed_slice())
}
