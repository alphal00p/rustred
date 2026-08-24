//! FORM-free Lorentz tensor reduction for vacuum integrals.
//!
//! The projector implemented here is global in all loop momenta.  For an
//! even-rank monomial it enumerates metric perfect matchings, constructs their
//! exact contraction Gram matrix over [`Coefficient`], and solves the matrix
//! once per rank.  Contracted loop-vector pairs remain typed scalar products;
//! converting those scalar products into denominator shifts belongs to the
//! family layer.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use symbolica::prelude::*;

use crate::coefficient::{Coefficient, CoefficientContext};

/// Largest naive perfect-matching basis enabled by default (rank eight).
///
/// Rank ten already has 945 matchings and calls for an orbit-reduced projector
/// rather than dense generic Gaussian elimination.  Callers can explicitly
/// raise this limit, but the default prevents an accidental cubic-size solve.
pub const DEFAULT_MAX_PROJECTOR_PAIRINGS: usize = 105;

/// Allocation and degree limits for constructing one legacy tensor monomial.
///
/// These limits are enforced while iterator inputs are consumed, before a new
/// vector, metric, or scalar-product map entry is retained.  The defaults
/// match the authenticated generic tensor projector's input policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorConstructionLimits {
    pub max_vectors: usize,
    pub max_metrics: usize,
    pub max_scalar_product_factor_entries: usize,
    pub max_distinct_scalar_products: usize,
    pub max_scalar_product_degree: u64,
    pub max_index_endpoints: usize,
}

impl TensorConstructionLimits {
    /// Compatibility policy for deprecated constructors that historically had
    /// no resource ceiling. New code should always pass an explicit policy.
    const UNBOUNDED: Self = Self {
        max_vectors: usize::MAX,
        max_metrics: usize::MAX,
        max_scalar_product_factor_entries: usize::MAX,
        max_distinct_scalar_products: usize::MAX,
        max_scalar_product_degree: u64::MAX,
        max_index_endpoints: usize::MAX,
    };
}

impl Default for TensorConstructionLimits {
    fn default() -> Self {
        Self {
            max_vectors: 4_096,
            max_metrics: 4_096,
            max_scalar_product_factor_entries: 4_096,
            max_distinct_scalar_products: 4_096,
            max_scalar_product_degree: u64::from(u16::MAX),
            max_index_endpoints: 16_384,
        }
    }
}

/// A stable, adapter-assigned Lorentz-index identifier.
///
/// An adapter may intern arbitrary Symbolica index atoms into these IDs and
/// retain the reverse map.  RustRed itself only needs equality and a canonical
/// order; it never interprets the numeric value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LorentzIndex(u32);

impl LorentzIndex {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u32 {
        self.0
    }

    pub fn to_atom(self) -> Atom {
        Atom::num(i64::from(self.0))
    }
}

impl From<u32> for LorentzIndex {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

/// A stable identifier for one of the integration loop momenta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LoopVector(u16);

impl LoopVector {
    pub const fn new(id: u16) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u16 {
        self.0
    }

    pub fn to_atom(self) -> Atom {
        Atom::num(i64::from(self.0))
    }
}

impl From<u16> for LoopVector {
    fn from(value: u16) -> Self {
        Self::new(value)
    }
}

/// A loop vector carrying one Lorentz index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexedVector {
    vector: LoopVector,
    index: LorentzIndex,
}

impl IndexedVector {
    pub const fn new(vector: LoopVector, index: LorentzIndex) -> Self {
        Self { vector, index }
    }

    pub const fn vector(self) -> LoopVector {
        self.vector
    }

    pub const fn index(self) -> LorentzIndex {
        self.index
    }
}

/// A symmetric Lorentz metric.  Its endpoints are stored canonically.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Metric {
    left: LorentzIndex,
    right: LorentzIndex,
}

impl Metric {
    pub fn new(left: LorentzIndex, right: LorentzIndex) -> Self {
        if left <= right {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }

    pub const fn left(self) -> LorentzIndex {
        self.left
    }

    pub const fn right(self) -> LorentzIndex {
        self.right
    }

    pub fn to_atom(self, metric_symbol: Symbol) -> Atom {
        FunctionBuilder::new(metric_symbol)
            .add_args([self.left.to_atom(), self.right.to_atom()])
            .finish()
    }
}

/// A symmetric scalar product of two integration loop momenta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScalarProduct {
    left: LoopVector,
    right: LoopVector,
}

impl ScalarProduct {
    pub fn new(left: LoopVector, right: LoopVector) -> Self {
        if left <= right {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }

    pub const fn left(self) -> LoopVector {
        self.left
    }

    pub const fn right(self) -> LoopVector {
        self.right
    }

    pub fn to_atom(self, scalar_product_symbol: Symbol) -> Atom {
        FunctionBuilder::new(scalar_product_symbol)
            .add_args([self.left.to_atom(), self.right.to_atom()])
            .finish()
    }
}

/// A canonical commutative monomial in loop scalar products.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScalarProductMonomial {
    factors: BTreeMap<ScalarProduct, u32>,
}

impl ScalarProductMonomial {
    pub fn one() -> Self {
        Self::default()
    }

    /// Construct with the default bounded policy.
    pub fn try_from_factors(
        factors: impl IntoIterator<Item = (ScalarProduct, u32)>,
    ) -> Result<Self, TensorError> {
        Self::try_from_factors_with_limits(factors, TensorConstructionLimits::default())
    }

    /// Construct with an explicit policy, checking each consumed entry before
    /// retaining a new scalar-product factor.
    pub fn try_from_factors_with_limits(
        factors: impl IntoIterator<Item = (ScalarProduct, u32)>,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        let mut result = Self::one();
        let mut consumed = 0usize;
        for (scalar_product, exponent) in factors {
            consumed = consumed
                .checked_add(1)
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor scalar-product constructor entries",
                })?;
            check_tensor_resource_limit(
                "tensor scalar-product constructor entries",
                consumed,
                limits.max_scalar_product_factor_entries,
            )?;
            result.try_multiply_power_with_limits(scalar_product, exponent, limits)?;
        }
        Ok(result)
    }

    /// Legacy unbounded constructor retained for source compatibility.
    ///
    /// New code must use [`Self::try_from_factors_with_limits`]. This wrapper
    /// can still panic when its historical `u32` exponent representation is
    /// exceeded because its return type cannot report construction failure.
    #[deprecated(
        since = "0.1.0",
        note = "use ScalarProductMonomial::try_from_factors_with_limits"
    )]
    pub fn from_factors(factors: impl IntoIterator<Item = (ScalarProduct, u32)>) -> Self {
        Self::try_from_factors_with_limits(factors, TensorConstructionLimits::UNBOUNDED)
            .expect("legacy scalar-product monomial exponent overflow")
    }

    pub fn factors(&self) -> &BTreeMap<ScalarProduct, u32> {
        &self.factors
    }

    pub fn exponent(&self, scalar_product: ScalarProduct) -> u32 {
        self.factors.get(&scalar_product).copied().unwrap_or(0)
    }

    /// Legacy lossy degree accessor.
    ///
    /// It saturates instead of panicking when the sum no longer fits `u32`.
    /// Exact code should use [`Self::checked_degree`].
    #[deprecated(since = "0.1.0", note = "use ScalarProductMonomial::checked_degree")]
    pub fn degree(&self) -> u32 {
        self.factors
            .values()
            .copied()
            .fold(0u32, u32::saturating_add)
    }

    pub fn checked_degree(&self) -> Result<u64, TensorError> {
        self.factors.values().try_fold(0u64, |degree, &exponent| {
            degree
                .checked_add(u64::from(exponent))
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor scalar-product degree",
                })
        })
    }

    pub fn is_one(&self) -> bool {
        self.factors.is_empty()
    }

    pub fn try_multiply(&mut self, scalar_product: ScalarProduct) -> Result<(), TensorError> {
        self.try_multiply_power_with_limits(scalar_product, 1, TensorConstructionLimits::default())
    }

    pub fn try_multiply_power(
        &mut self,
        scalar_product: ScalarProduct,
        exponent: u32,
    ) -> Result<(), TensorError> {
        self.try_multiply_power_with_limits(
            scalar_product,
            exponent,
            TensorConstructionLimits::default(),
        )
    }

    /// Multiply by a power under an explicit resource policy.
    ///
    /// All checks precede insertion or exponent mutation, so every failure is
    /// transactional and leaves `self` unchanged.
    pub fn try_multiply_power_with_limits(
        &mut self,
        scalar_product: ScalarProduct,
        exponent: u32,
        limits: TensorConstructionLimits,
    ) -> Result<(), TensorError> {
        if exponent == 0 {
            return Ok(());
        }
        let current = self.factors.get(&scalar_product).copied().unwrap_or(0);
        let updated = current
            .checked_add(exponent)
            .ok_or(TensorError::ScalarProductExponentOverflow { scalar_product })?;
        let degree = self
            .checked_degree()?
            .checked_add(u64::from(exponent))
            .ok_or(TensorError::ResourceCountOverflow {
                resource: "tensor scalar-product degree",
            })?;
        if degree > limits.max_scalar_product_degree {
            return Err(TensorError::ScalarProductDegreeLimit {
                requested: degree,
                limit: limits.max_scalar_product_degree,
            });
        }
        if current == 0 {
            let factors =
                self.factors
                    .len()
                    .checked_add(1)
                    .ok_or(TensorError::ResourceCountOverflow {
                        resource: "tensor distinct scalar products",
                    })?;
            check_tensor_resource_limit(
                "tensor distinct scalar products",
                factors,
                limits.max_distinct_scalar_products,
            )?;
        }
        self.factors.insert(scalar_product, updated);
        Ok(())
    }

    pub fn try_multiply_monomial_with_limits(
        &mut self,
        other: &Self,
        limits: TensorConstructionLimits,
    ) -> Result<(), TensorError> {
        let combined_degree = self
            .checked_degree()?
            .checked_add(other.checked_degree()?)
            .ok_or(TensorError::ResourceCountOverflow {
                resource: "tensor scalar-product degree",
            })?;
        if combined_degree > limits.max_scalar_product_degree {
            return Err(TensorError::ScalarProductDegreeLimit {
                requested: combined_degree,
                limit: limits.max_scalar_product_degree,
            });
        }
        let new_factors = other
            .factors
            .keys()
            .filter(|factor| !self.factors.contains_key(factor))
            .count();
        let combined_factors = self.factors.len().checked_add(new_factors).ok_or(
            TensorError::ResourceCountOverflow {
                resource: "tensor distinct scalar products",
            },
        )?;
        check_tensor_resource_limit(
            "tensor distinct scalar products",
            combined_factors,
            limits.max_distinct_scalar_products,
        )?;
        for (&scalar_product, &exponent) in &other.factors {
            let current = self.factors.get(&scalar_product).copied().unwrap_or(0);
            current
                .checked_add(exponent)
                .ok_or(TensorError::ScalarProductExponentOverflow { scalar_product })?;
        }
        for (&scalar_product, &exponent) in &other.factors {
            let current = self.factors.get(&scalar_product).copied().unwrap_or(0);
            let updated = current
                .checked_add(exponent)
                .ok_or(TensorError::ScalarProductExponentOverflow { scalar_product })?;
            self.factors.insert(scalar_product, updated);
        }
        Ok(())
    }

    /// Deprecated compatibility mutator. Unlike the historical implementation,
    /// exponent overflow is returned as a typed error instead of panicking.
    #[deprecated(
        since = "0.1.0",
        note = "use ScalarProductMonomial::try_multiply_power_with_limits"
    )]
    pub fn multiply_power(
        &mut self,
        scalar_product: ScalarProduct,
        exponent: u32,
    ) -> Result<(), TensorError> {
        self.try_multiply_power_with_limits(
            scalar_product,
            exponent,
            TensorConstructionLimits::UNBOUNDED,
        )
    }

    #[deprecated(since = "0.1.0", note = "use ScalarProductMonomial::try_multiply")]
    pub fn multiply(&mut self, scalar_product: ScalarProduct) -> Result<(), TensorError> {
        self.try_multiply_power_with_limits(scalar_product, 1, TensorConstructionLimits::UNBOUNDED)
    }

    #[deprecated(
        since = "0.1.0",
        note = "use ScalarProductMonomial::try_multiply_monomial_with_limits"
    )]
    pub fn multiply_monomial(&mut self, other: &Self) -> Result<(), TensorError> {
        self.try_multiply_monomial_with_limits(other, TensorConstructionLimits::UNBOUNDED)
    }

    pub fn to_atom(&self, scalar_product_symbol: Symbol) -> Atom {
        self.factors
            .iter()
            .fold(Atom::num(1), |product, (&factor, &exponent)| {
                let factor = factor.to_atom(scalar_product_symbol);
                let factor = if exponent == 1 {
                    factor
                } else {
                    factor.pow(Atom::num(i64::from(exponent)))
                };
                product * factor
            })
    }
}

/// A canonical product of metrics carrying the free output indices.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct MetricPairing {
    metrics: Vec<Metric>,
}

impl MetricPairing {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(metrics: impl IntoIterator<Item = Metric>) -> Self {
        let mut metrics: Vec<_> = metrics.into_iter().collect();
        metrics.sort_unstable();
        Self { metrics }
    }

    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    pub fn is_empty(&self) -> bool {
        self.metrics.is_empty()
    }

    fn extended_with(&self, metrics: impl IntoIterator<Item = Metric>) -> Self {
        let mut result = self.metrics.clone();
        result.extend(metrics);
        result.sort_unstable();
        Self { metrics: result }
    }

    pub fn to_atom(&self, metric_symbol: Symbol) -> Atom {
        self.metrics
            .iter()
            .copied()
            .fold(Atom::num(1), |product, metric| {
                product * metric.to_atom(metric_symbol)
            })
    }
}

/// One tensor numerator monomial before metric contraction and projection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TensorMonomial {
    vectors: Vec<IndexedVector>,
    metrics: Vec<Metric>,
    scalar_products: ScalarProductMonomial,
}

impl TensorMonomial {
    pub fn try_new(vectors: impl IntoIterator<Item = IndexedVector>) -> Result<Self, TensorError> {
        Self::try_new_with_limits(vectors, TensorConstructionLimits::default())
    }

    pub fn try_new_with_limits(
        vectors: impl IntoIterator<Item = IndexedVector>,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        Self::try_from_parts_with_limits(vectors, [], ScalarProductMonomial::one(), limits)
    }

    /// Legacy constructor retained for source compatibility. It collects the
    /// complete iterator before any caller-side projector limit can run.
    #[deprecated(since = "0.1.0", note = "use TensorMonomial::try_new_with_limits")]
    pub fn new(vectors: impl IntoIterator<Item = IndexedVector>) -> Self {
        Self {
            vectors: vectors.into_iter().collect(),
            ..Self::default()
        }
    }

    #[deprecated(
        since = "0.1.0",
        note = "use TensorMonomial::try_from_parts_with_limits"
    )]
    pub fn from_parts(
        vectors: impl IntoIterator<Item = IndexedVector>,
        metrics: impl IntoIterator<Item = Metric>,
        scalar_products: ScalarProductMonomial,
    ) -> Self {
        Self {
            vectors: vectors.into_iter().collect(),
            metrics: metrics.into_iter().collect(),
            scalar_products,
        }
    }

    pub fn try_from_parts(
        vectors: impl IntoIterator<Item = IndexedVector>,
        metrics: impl IntoIterator<Item = Metric>,
        scalar_products: ScalarProductMonomial,
    ) -> Result<Self, TensorError> {
        Self::try_from_parts_with_limits(
            vectors,
            metrics,
            scalar_products,
            TensorConstructionLimits::default(),
        )
    }

    /// Bounded constructor for all retained tensor source structures.
    pub fn try_from_parts_with_limits(
        vectors: impl IntoIterator<Item = IndexedVector>,
        metrics: impl IntoIterator<Item = Metric>,
        scalar_products: ScalarProductMonomial,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        validate_scalar_product_monomial(&scalar_products, limits)?;
        let vectors = bounded_tensor_vectors(vectors, limits)?;
        let metrics = bounded_tensor_metrics(metrics, vectors.len(), limits)?;
        validate_index_endpoint_count(vectors.len(), metrics.len(), limits)?;
        Ok(Self {
            vectors,
            metrics,
            scalar_products,
        })
    }

    pub fn vectors(&self) -> &[IndexedVector] {
        &self.vectors
    }

    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    pub fn scalar_products(&self) -> &ScalarProductMonomial {
        &self.scalar_products
    }

    #[deprecated(
        since = "0.1.0",
        note = "use TensorMonomial::try_with_vector_with_limits"
    )]
    pub fn with_vector(mut self, vector: IndexedVector) -> Self {
        self.vectors.push(vector);
        self
    }

    #[deprecated(
        since = "0.1.0",
        note = "use TensorMonomial::try_with_metric_with_limits"
    )]
    pub fn with_metric(mut self, metric: Metric) -> Self {
        self.metrics.push(metric);
        self
    }

    pub fn try_with_vector_with_limits(
        mut self,
        vector: IndexedVector,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        let vectors =
            self.vectors
                .len()
                .checked_add(1)
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor constructor vectors",
                })?;
        check_tensor_resource_limit("tensor constructor vectors", vectors, limits.max_vectors)?;
        validate_index_endpoint_count(vectors, self.metrics.len(), limits)?;
        self.vectors.push(vector);
        Ok(self)
    }

    pub fn try_with_metric_with_limits(
        mut self,
        metric: Metric,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        let metrics =
            self.metrics
                .len()
                .checked_add(1)
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor constructor metrics",
                })?;
        check_tensor_resource_limit("tensor constructor metrics", metrics, limits.max_metrics)?;
        validate_index_endpoint_count(self.vectors.len(), metrics, limits)?;
        self.metrics.push(metric);
        Ok(self)
    }

    pub fn try_with_scalar_product_with_limits(
        mut self,
        scalar_product: ScalarProduct,
        limits: TensorConstructionLimits,
    ) -> Result<Self, TensorError> {
        self.scalar_products
            .try_multiply_power_with_limits(scalar_product, 1, limits)?;
        Ok(self)
    }

    /// Deprecated compatibility builder. Exponent overflow is now returned as
    /// a typed error rather than panicking.
    #[deprecated(
        since = "0.1.0",
        note = "use TensorMonomial::try_with_scalar_product_with_limits"
    )]
    pub fn with_scalar_product(
        mut self,
        scalar_product: ScalarProduct,
    ) -> Result<Self, TensorError> {
        self.scalar_products.try_multiply_power_with_limits(
            scalar_product,
            1,
            TensorConstructionLimits::UNBOUNDED,
        )?;
        Ok(self)
    }
}

/// The exact result of eliminating all dummy indices carried by input metrics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetricContraction {
    coefficient: Coefficient,
    vectors: Vec<IndexedVector>,
    metrics: MetricPairing,
    scalar_products: ScalarProductMonomial,
}

impl MetricContraction {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn vectors(&self) -> &[IndexedVector] {
        &self.vectors
    }

    pub fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub fn scalar_products(&self) -> &ScalarProductMonomial {
        &self.scalar_products
    }
}

/// One canonical term in a projected tensor result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorTerm {
    coefficient: Coefficient,
    metrics: MetricPairing,
    scalar_products: ScalarProductMonomial,
}

impl TensorTerm {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub fn scalar_products(&self) -> &ScalarProductMonomial {
        &self.scalar_products
    }

    pub fn to_atom(&self, metric_symbol: Symbol, scalar_product_symbol: Symbol) -> Atom {
        self.coefficient.to_expression()
            * self.metrics.to_atom(metric_symbol)
            * self.scalar_products.to_atom(scalar_product_symbol)
    }
}

/// A sparse, deterministic sum of projected tensor structures.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TensorReduction {
    terms: Vec<TensorTerm>,
}

impl TensorReduction {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn terms(&self) -> &[TensorTerm] {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn len(&self) -> usize {
        self.terms.len()
    }

    pub fn coefficient(
        &self,
        metrics: &MetricPairing,
        scalar_products: &ScalarProductMonomial,
    ) -> Option<&Coefficient> {
        self.terms
            .iter()
            .find(|term| &term.metrics == metrics && &term.scalar_products == scalar_products)
            .map(|term| &term.coefficient)
    }

    pub fn to_atom(&self, metric_symbol: Symbol, scalar_product_symbol: Symbol) -> Atom {
        self.terms.iter().fold(Atom::num(0), |sum, term| {
            sum + term.to_atom(metric_symbol, scalar_product_symbol)
        })
    }
}

/// A perfect matching of the numbered slots `0..rank`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SlotPairing {
    rank: usize,
    pairs: Vec<(usize, usize)>,
}

impl SlotPairing {
    pub fn new(
        rank: usize,
        pairs: impl IntoIterator<Item = (usize, usize)>,
    ) -> Result<Self, TensorError> {
        let mut pairs: Vec<_> = pairs
            .into_iter()
            .map(|(left, right)| {
                if left <= right {
                    (left, right)
                } else {
                    (right, left)
                }
            })
            .collect();
        pairs.sort_unstable();

        if pairs.len().saturating_mul(2) != rank {
            return Err(TensorError::InvalidPairingSize {
                rank,
                pairs: pairs.len(),
            });
        }

        let mut used = BTreeSet::new();
        for &(left, right) in &pairs {
            if left >= rank || right >= rank {
                return Err(TensorError::PairingSlotOutOfRange { rank, left, right });
            }
            if left == right {
                return Err(TensorError::RepeatedPairingSlot { rank, slot: left });
            }
            if !used.insert(left) {
                return Err(TensorError::RepeatedPairingSlot { rank, slot: left });
            }
            if !used.insert(right) {
                return Err(TensorError::RepeatedPairingSlot { rank, slot: right });
            }
        }

        Ok(Self { rank, pairs })
    }

    pub const fn rank(&self) -> usize {
        self.rank
    }

    pub fn pairs(&self) -> &[(usize, usize)] {
        &self.pairs
    }

    /// Number of closed index loops obtained by contracting two pairings.
    pub fn contraction_cycles(&self, other: &Self) -> Result<usize, TensorError> {
        if self.rank != other.rank {
            return Err(TensorError::MismatchedPairingRanks {
                left: self.rank,
                right: other.rank,
            });
        }
        if self.rank == 0 {
            return Ok(0);
        }

        let mut components = DisjointSet::new(self.rank);
        for &(left, right) in self.pairs.iter().chain(&other.pairs) {
            components.union(left, right);
        }
        Ok((0..self.rank)
            .filter(|&slot| components.find(slot) == slot)
            .count())
    }

    fn from_generated(rank: usize, pairs: Vec<(usize, usize)>) -> Self {
        debug_assert_eq!(pairs.len() * 2, rank);
        Self { rank, pairs }
    }
}

/// Return `(rank - 1)!!`, the number of perfect matchings of an even rank.
/// Odd ranks have no perfect matchings.  `None` denotes `usize` overflow.
pub fn perfect_matching_count(rank: usize) -> Option<usize> {
    if rank % 2 == 1 {
        return Some(0);
    }
    (1..rank)
        .step_by(2)
        .try_fold(1usize, |count, factor| count.checked_mul(factor))
}

/// Enumerate all perfect matchings in deterministic lexicographic order.
pub fn perfect_matchings(
    rank: usize,
    max_pairings: usize,
) -> Result<Vec<SlotPairing>, TensorError> {
    let Some(pairing_count) = perfect_matching_count(rank) else {
        return Err(TensorError::PairingLimitExceeded {
            rank,
            pairings: None,
            limit: max_pairings,
        });
    };
    if pairing_count > max_pairings {
        return Err(TensorError::PairingLimitExceeded {
            rank,
            pairings: Some(pairing_count),
            limit: max_pairings,
        });
    }
    if rank % 2 == 1 {
        return Ok(Vec::new());
    }

    let mut output = Vec::with_capacity(pairing_count);
    let mut current = Vec::with_capacity(rank / 2);
    let remaining: Vec<_> = (0..rank).collect();
    enumerate_pairings(rank, &remaining, &mut current, &mut output);
    Ok(output)
}

/// Errors emitted by metric contraction or vacuum projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TensorError {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ScalarProductExponentOverflow {
        scalar_product: ScalarProduct,
    },
    ScalarProductDegreeLimit {
        requested: u64,
        limit: u64,
    },
    UnknownDimensionParameter(String),
    InvalidPairingSize {
        rank: usize,
        pairs: usize,
    },
    PairingSlotOutOfRange {
        rank: usize,
        left: usize,
        right: usize,
    },
    RepeatedPairingSlot {
        rank: usize,
        slot: usize,
    },
    MismatchedPairingRanks {
        left: usize,
        right: usize,
    },
    PairingLimitExceeded {
        rank: usize,
        pairings: Option<usize>,
        limit: usize,
    },
    InvalidIndexMultiplicity {
        index: LorentzIndex,
        occurrences: usize,
    },
    InvalidMetricComponent {
        vectors: usize,
        free_indices: usize,
    },
    SingularProjector {
        rank: usize,
        column: usize,
    },
}

impl fmt::Display for TensorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} requires {requested} entries, above limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ScalarProductExponentOverflow { scalar_product } => write!(
                formatter,
                "scalar-product exponent overflow for loop momenta {} and {}",
                scalar_product.left().id(),
                scalar_product.right().id()
            ),
            Self::ScalarProductDegreeLimit { requested, limit } => write!(
                formatter,
                "tensor scalar-product degree {requested} exceeds limit {limit}"
            ),
            Self::UnknownDimensionParameter(name) => {
                write!(formatter, "unknown dimension parameter `{name}`")
            }
            Self::InvalidPairingSize { rank, pairs } => write!(
                formatter,
                "rank-{rank} pairing has {pairs} pairs instead of {}",
                rank / 2
            ),
            Self::PairingSlotOutOfRange { rank, left, right } => write!(
                formatter,
                "pair ({left}, {right}) contains a slot outside rank {rank}"
            ),
            Self::RepeatedPairingSlot { rank, slot } => {
                write!(
                    formatter,
                    "slot {slot} is repeated in a rank-{rank} pairing"
                )
            }
            Self::MismatchedPairingRanks { left, right } => write!(
                formatter,
                "cannot contract rank-{left} and rank-{right} pairings"
            ),
            Self::PairingLimitExceeded {
                rank,
                pairings,
                limit,
            } => match pairings {
                Some(pairings) => write!(
                    formatter,
                    "rank-{rank} needs {pairings} perfect matchings, above limit {limit}"
                ),
                None => write!(
                    formatter,
                    "the rank-{rank} perfect-matching count overflows usize (limit {limit})"
                ),
            },
            Self::InvalidIndexMultiplicity { index, occurrences } => write!(
                formatter,
                "Lorentz index {} occurs {occurrences} times; expected once or twice",
                index.id()
            ),
            Self::InvalidMetricComponent {
                vectors,
                free_indices,
            } => write!(
                formatter,
                "invalid metric-contraction component with {vectors} vector endpoints and \
                 {free_indices} free indices"
            ),
            Self::SingularProjector { rank, column } => write!(
                formatter,
                "rank-{rank} metric Gram matrix is singular at pivot column {column}"
            ),
        }
    }
}

impl Error for TensorError {}

fn check_tensor_resource_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TensorError> {
    if requested > limit {
        Err(TensorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bounded_tensor_vectors(
    values: impl IntoIterator<Item = IndexedVector>,
    limits: TensorConstructionLimits,
) -> Result<Vec<IndexedVector>, TensorError> {
    let mut retained = Vec::new();
    for value in values {
        let requested =
            retained
                .len()
                .checked_add(1)
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor constructor vectors",
                })?;
        check_tensor_resource_limit("tensor constructor vectors", requested, limits.max_vectors)?;
        check_tensor_resource_limit(
            "tensor constructor index endpoints",
            requested,
            limits.max_index_endpoints,
        )?;
        retained.push(value);
    }
    Ok(retained)
}

fn bounded_tensor_metrics(
    values: impl IntoIterator<Item = Metric>,
    vectors: usize,
    limits: TensorConstructionLimits,
) -> Result<Vec<Metric>, TensorError> {
    let mut retained = Vec::new();
    for value in values {
        let requested =
            retained
                .len()
                .checked_add(1)
                .ok_or(TensorError::ResourceCountOverflow {
                    resource: "tensor constructor metrics",
                })?;
        check_tensor_resource_limit("tensor constructor metrics", requested, limits.max_metrics)?;
        validate_index_endpoint_count(vectors, requested, limits)?;
        retained.push(value);
    }
    Ok(retained)
}

fn validate_scalar_product_monomial(
    scalar_products: &ScalarProductMonomial,
    limits: TensorConstructionLimits,
) -> Result<(), TensorError> {
    check_tensor_resource_limit(
        "tensor distinct scalar products",
        scalar_products.factors.len(),
        limits.max_distinct_scalar_products,
    )?;
    let degree = scalar_products.checked_degree()?;
    if degree > limits.max_scalar_product_degree {
        return Err(TensorError::ScalarProductDegreeLimit {
            requested: degree,
            limit: limits.max_scalar_product_degree,
        });
    }
    Ok(())
}

fn validate_index_endpoint_count(
    vectors: usize,
    metrics: usize,
    limits: TensorConstructionLimits,
) -> Result<(), TensorError> {
    let metric_endpoints = metrics
        .checked_mul(2)
        .ok_or(TensorError::ResourceCountOverflow {
            resource: "tensor constructor index endpoints",
        })?;
    let endpoints =
        vectors
            .checked_add(metric_endpoints)
            .ok_or(TensorError::ResourceCountOverflow {
                resource: "tensor constructor index endpoints",
            })?;
    check_tensor_resource_limit(
        "tensor constructor index endpoints",
        endpoints,
        limits.max_index_endpoints,
    )
}

/// Dense exact global O(d) projector, cached by tensor rank.
#[derive(Debug)]
pub struct VacuumTensorProjector {
    coefficients: CoefficientContext,
    dimension: Coefficient,
    max_pairings: usize,
    cache: BTreeMap<usize, ProjectorData>,
}

impl VacuumTensorProjector {
    pub fn new(
        coefficients: &CoefficientContext,
        dimension_parameter: &str,
    ) -> Result<Self, TensorError> {
        let dimension = coefficients.parameter(dimension_parameter).ok_or_else(|| {
            TensorError::UnknownDimensionParameter(dimension_parameter.to_owned())
        })?;
        Ok(Self::with_dimension(coefficients, dimension))
    }

    pub fn with_dimension(coefficients: &CoefficientContext, dimension: Coefficient) -> Self {
        Self {
            coefficients: coefficients.clone(),
            dimension,
            max_pairings: DEFAULT_MAX_PROJECTOR_PAIRINGS,
            cache: BTreeMap::new(),
        }
    }

    pub fn with_max_pairings(mut self, max_pairings: usize) -> Self {
        self.max_pairings = max_pairings;
        self
    }

    pub fn max_pairings(&self) -> usize {
        self.max_pairings
    }

    pub fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    /// Exact `d^cycles(P union Q)` contraction used in the projector Gram matrix.
    pub fn pairing_contraction(
        &self,
        left: &SlotPairing,
        right: &SlotPairing,
    ) -> Result<Coefficient, TensorError> {
        Ok(self.dimension.pow(left.contraction_cycles(right)? as u64))
    }

    /// Eliminate dummy indices from metrics, metric chains, and vector pairs.
    ///
    /// Valid Einstein monomials may use an index once (free) or twice (dummy).
    /// A closed metric component contributes `d`, a component ending on two
    /// vectors contributes their scalar product, and open metric chains are
    /// shortened to a single canonical metric or indexed vector.
    pub fn contract_metrics(
        &self,
        input: &TensorMonomial,
    ) -> Result<MetricContraction, TensorError> {
        #[derive(Default)]
        struct IndexData {
            occurrences: usize,
            vectors: Vec<LoopVector>,
        }

        let mut index_data = BTreeMap::<LorentzIndex, IndexData>::new();
        for vector in &input.vectors {
            let data = index_data.entry(vector.index).or_default();
            data.occurrences += 1;
            data.vectors.push(vector.vector);
        }
        for metric in &input.metrics {
            for index in [metric.left, metric.right] {
                index_data.entry(index).or_default().occurrences += 1;
            }
        }
        for (&index, data) in &index_data {
            if data.occurrences > 2 {
                return Err(TensorError::InvalidIndexMultiplicity {
                    index,
                    occurrences: data.occurrences,
                });
            }
        }

        let indices: Vec<_> = index_data.keys().copied().collect();
        let positions: BTreeMap<_, _> = indices
            .iter()
            .copied()
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect();
        let mut components = DisjointSet::new(indices.len());
        for metric in &input.metrics {
            components.union(positions[&metric.left], positions[&metric.right]);
        }

        #[derive(Default)]
        struct Component {
            vectors: Vec<LoopVector>,
            free_indices: Vec<LorentzIndex>,
        }

        let mut grouped = BTreeMap::<usize, Component>::new();
        for (position, &index) in indices.iter().enumerate() {
            let data = &index_data[&index];
            let component = grouped.entry(components.find(position)).or_default();
            component.vectors.extend(data.vectors.iter().copied());
            if data.occurrences == 1 {
                component.free_indices.push(index);
            }
        }

        let mut closed_loops = 0u64;
        let mut vectors = Vec::new();
        let mut metrics = Vec::new();
        let mut scalar_products = input.scalar_products.clone();
        for mut component in grouped.into_values() {
            component.vectors.sort_unstable();
            component.free_indices.sort_unstable();
            match (
                component.vectors.as_slice(),
                component.free_indices.as_slice(),
            ) {
                ([], []) => closed_loops += 1,
                ([], &[left, right]) => metrics.push(Metric::new(left, right)),
                (&[vector], &[index]) => vectors.push(IndexedVector::new(vector, index)),
                (&[left, right], []) => {
                    scalar_products.try_multiply_power_with_limits(
                        ScalarProduct::new(left, right),
                        1,
                        TensorConstructionLimits::UNBOUNDED,
                    )?;
                }
                (vectors, free_indices) => {
                    return Err(TensorError::InvalidMetricComponent {
                        vectors: vectors.len(),
                        free_indices: free_indices.len(),
                    });
                }
            }
        }
        vectors.sort_unstable();

        Ok(MetricContraction {
            coefficient: self.dimension.pow(closed_loops),
            vectors,
            metrics: MetricPairing::new(metrics),
            scalar_products,
        })
    }

    /// Project a vacuum tensor monomial onto the global O(d)-invariant basis.
    ///
    /// Rank zero is the identity and odd loop-vector rank is exactly zero.
    /// Even ranks use all perfect matchings up to the configured resource limit.
    pub fn reduce(&mut self, input: &TensorMonomial) -> Result<TensorReduction, TensorError> {
        let contracted = self.contract_metrics(input)?;
        let rank = contracted.vectors.len();
        if rank % 2 == 1 {
            return Ok(TensorReduction::zero());
        }
        if rank == 0 {
            return Ok(TensorReduction {
                terms: vec![TensorTerm {
                    coefficient: contracted.coefficient,
                    metrics: contracted.metrics,
                    scalar_products: contracted.scalar_products,
                }],
            });
        }

        let vectors = contracted.vectors;
        let existing_metrics = contracted.metrics;
        let existing_scalar_products = contracted.scalar_products;
        let metric_factor = contracted.coefficient;
        let projector = self.projector_data(rank)?;
        let mut terms = BTreeMap::<TensorStructure, Coefficient>::new();

        for (output_position, output_pairing) in projector.pairings.iter().enumerate() {
            let metrics = existing_metrics.extended_with(
                output_pairing
                    .pairs
                    .iter()
                    .map(|&(left, right)| Metric::new(vectors[left].index, vectors[right].index)),
            );
            for (source_position, source_pairing) in projector.pairings.iter().enumerate() {
                let mut scalar_products = existing_scalar_products.clone();
                for &(left, right) in &source_pairing.pairs {
                    scalar_products.try_multiply_power_with_limits(
                        ScalarProduct::new(vectors[left].vector, vectors[right].vector),
                        1,
                        TensorConstructionLimits::UNBOUNDED,
                    )?;
                }
                let coefficient =
                    &metric_factor * &projector.inverse_gram[output_position][source_position];
                add_tensor_structure(
                    &mut terms,
                    TensorStructure {
                        metrics: metrics.clone(),
                        scalar_products,
                    },
                    coefficient,
                );
            }
        }

        Ok(TensorReduction {
            terms: terms
                .into_iter()
                .map(|(structure, coefficient)| TensorTerm {
                    coefficient,
                    metrics: structure.metrics,
                    scalar_products: structure.scalar_products,
                })
                .collect(),
        })
    }

    fn projector_data(&mut self, rank: usize) -> Result<&ProjectorData, TensorError> {
        if !self.cache.contains_key(&rank) {
            let data = self.build_projector(rank)?;
            self.cache.insert(rank, data);
        }
        Ok(self
            .cache
            .get(&rank)
            .expect("projector was inserted immediately above"))
    }

    fn build_projector(&self, rank: usize) -> Result<ProjectorData, TensorError> {
        let pairings = perfect_matchings(rank, self.max_pairings)?;
        let mut gram = Vec::with_capacity(pairings.len());
        for left in &pairings {
            let mut row = Vec::with_capacity(pairings.len());
            for right in &pairings {
                row.push(self.pairing_contraction(left, right)?);
            }
            gram.push(row);
        }
        let inverse_gram = invert_matrix(&self.coefficients, gram, rank)?;
        Ok(ProjectorData {
            pairings,
            inverse_gram,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TensorStructure {
    metrics: MetricPairing,
    scalar_products: ScalarProductMonomial,
}

#[derive(Debug)]
struct ProjectorData {
    pairings: Vec<SlotPairing>,
    inverse_gram: Vec<Vec<Coefficient>>,
}

fn add_tensor_structure(
    terms: &mut BTreeMap<TensorStructure, Coefficient>,
    structure: TensorStructure,
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    if let Some(current) = terms.get_mut(&structure) {
        let sum = &*current + &coefficient;
        if sum.is_zero() {
            terms.remove(&structure);
        } else {
            *current = sum;
        }
    } else {
        terms.insert(structure, coefficient);
    }
}

fn enumerate_pairings(
    rank: usize,
    remaining: &[usize],
    current: &mut Vec<(usize, usize)>,
    output: &mut Vec<SlotPairing>,
) {
    if remaining.is_empty() {
        output.push(SlotPairing::from_generated(rank, current.clone()));
        return;
    }

    let left = remaining[0];
    for partner_position in 1..remaining.len() {
        let right = remaining[partner_position];
        current.push((left, right));
        let mut next = Vec::with_capacity(remaining.len() - 2);
        next.extend_from_slice(&remaining[1..partner_position]);
        next.extend_from_slice(&remaining[partner_position + 1..]);
        enumerate_pairings(rank, &next, current, output);
        current.pop();
    }
}

fn invert_matrix(
    coefficients: &CoefficientContext,
    matrix: Vec<Vec<Coefficient>>,
    rank: usize,
) -> Result<Vec<Vec<Coefficient>>, TensorError> {
    let size = matrix.len();
    debug_assert!(matrix.iter().all(|row| row.len() == size));
    let mut augmented = Vec::with_capacity(size);
    for (row_position, mut row) in matrix.into_iter().enumerate() {
        row.extend((0..size).map(|column| {
            if column == row_position {
                coefficients.one()
            } else {
                coefficients.zero()
            }
        }));
        augmented.push(row);
    }

    for column in 0..size {
        let Some(pivot) = (column..size).find(|&row| !augmented[row][column].is_zero()) else {
            return Err(TensorError::SingularProjector { rank, column });
        };
        augmented.swap(column, pivot);

        let pivot = augmented[column][column].clone();
        for entry in &mut augmented[column] {
            *entry = &*entry / &pivot;
        }
        let pivot_row = augmented[column].clone();

        for (row_position, row) in augmented.iter_mut().enumerate() {
            if row_position == column {
                continue;
            }
            let factor = row[column].clone();
            if factor.is_zero() {
                continue;
            }
            for (entry, pivot_entry) in row.iter_mut().zip(&pivot_row) {
                *entry = &*entry - &(&factor * pivot_entry);
            }
        }
    }

    Ok(augmented
        .into_iter()
        .map(|row| row.into_iter().skip(size).collect())
        .collect())
}

#[derive(Debug)]
struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, value: usize) -> usize {
        let parent = self.parent[value];
        if parent == value {
            value
        } else {
            let root = self.find(parent);
            self.parent[value] = root;
            root
        }
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            let (root, child) = if left < right {
                (left, right)
            } else {
                (right, left)
            };
            self.parent[child] = root;
        }
    }
}
