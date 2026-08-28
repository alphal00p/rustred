//! Authenticated, FORM-free tensor projection for generic vacuum families.
//!
//! This module is the proof-bearing replacement for passing an unauthenticated
//! [`TensorReduction`](crate::TensorReduction) into the generic family bridge.
//! A projection starts from the original [`TensorMonomial`](crate::TensorMonomial)
//! and an [`IntegralFamily`](crate::IntegralFamily).  The Lorentz dimension is
//! taken exclusively from [`IntegralFamily::dimension`], loop-vector ids are
//! checked against the family's ordered loop basis, and every exact Symbolica
//! operation is performed through the checked [`CoefficientContext`](crate::algebra::CoefficientContext)
//! API.
//!
//! Only vacuum projection is claimed here.  A tensor monomial containing
//! external indexed vectors needs a larger covariant basis (metrics and
//! external vectors) and cannot be represented by the legacy
//! [`TensorMonomial`](crate::TensorMonomial) type.  Families with external
//! momenta are therefore rejected explicitly instead of being projected with
//! a vacuum formula.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::algebra::matrix::{
    SymbolicaCoefficientMatrixError, SymbolicaCoefficientMatrixLimits,
    SymbolicaCoefficientMatrixStats, invert_and_verify_coefficient_matrix, power_of_coefficient,
};
use crate::generic_family::BasePolynomial;
use crate::{
    ConcreteIntegralKey, FamilyDomain, GenericTensorFamilyError, GenericTensorFamilyLimits,
    GenericTensorFamilyReducer, GenericTensorIntegralReduction, GenericTensorNumerator,
    GenericTensorTerm, IndexedVector, IntegralFamily, LoopVector, LorentzIndex, Metric,
    MetricPairing, ScalarProductCoordinate, SlotPairing, TensorError, TensorMonomial,
    algebra::Coefficient, algebra::CoefficientContext, algebra::ExactAlgebraError,
    algebra::ExactAlgebraLimits, perfect_matching_count, perfect_matchings,
};

/// Stable semantic version of the authenticated vacuum projector.
pub const GENERIC_VACUUM_TENSOR_PROJECTION_V1_SCHEMA: &str =
    "rustred-generic-vacuum-tensor-projection-v1";

/// Current semantic version of the authenticated vacuum projector.
///
/// V2 delegates the dense Gram inverse to Symbolica, authenticates it on both
/// sides, and records the basis-independent Gram-determinant guard rather than
/// an elimination-pivot transcript.
pub const GENERIC_VACUUM_TENSOR_PROJECTION_V2_SCHEMA: &str =
    "rustred-generic-vacuum-tensor-projection-v2";

/// Stable semantic version of the spectator-covariant vacuum projector.
pub const GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V1_SCHEMA: &str =
    "rustred-generic-vacuum-covariant-tensor-projection-v1";

/// Current semantic version of the spectator-covariant vacuum projector.
pub const GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V2_SCHEMA: &str =
    "rustred-generic-vacuum-covariant-tensor-projection-v2";

/// Stable semantic version of projection plus scalar-family lowering.
pub const AUTHENTICATED_VACUUM_TENSOR_LOWERING_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-tensor-lowering-v1";

/// Current semantic version of projection plus scalar-family lowering.
pub const AUTHENTICATED_VACUUM_TENSOR_LOWERING_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-tensor-lowering-v2";

/// Resource policy for one authenticated projection.
///
/// The limits cover both allocations and aggregate work.  In particular,
/// `max_pairings` is not the only relevant ceiling: the Gram matrix, augmented
/// elimination matrix, generated tensor structures, guard provenance, and
/// exact arithmetic are independently bounded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericTensorProjectorLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_rank: usize,
    pub max_pairings: usize,
    pub max_input_vectors: usize,
    pub max_input_spectator_vectors: usize,
    pub max_input_metrics: usize,
    pub max_input_scalar_product_factors: usize,
    pub max_input_spectator_scalar_product_factors: usize,
    pub max_scalar_product_degree: u64,
    pub max_spectator_scalar_product_degree: u64,
    pub max_index_endpoints: usize,
    pub max_gram_entries: usize,
    pub max_augmented_entries: usize,
    /// Largest conservative simultaneously-live Symbolica matrix payload.
    /// Dense inversion authenticates the source, inverse, and one replay
    /// product while Symbolica's augmented workspace is live.
    pub max_matrix_live_entries: usize,
    /// Aggregate clone-owned bytes copied into authenticated Symbolica
    /// coefficient-algebra sessions (powers and the Gram inverse).
    pub max_matrix_input_retained_bytes: usize,
    /// Aggregate clone-owned bytes authenticated in Symbolica coefficient
    /// outputs, including determinant and two-sided inverse replay products.
    pub max_matrix_output_retained_bytes: usize,
    pub max_projection_candidates: usize,
    pub max_output_terms: usize,
    pub max_output_structure_entries: usize,
    pub max_arithmetic_operations: u64,
    pub max_structural_operations: u64,
    pub max_nonzero_conditions: usize,
    pub max_guard_origins_per_condition: usize,
    pub max_guard_origins: usize,
    pub max_guard_polynomial_terms: usize,
    pub max_guard_exponent_entries: usize,
    pub max_family_domain_conditions: usize,
    pub max_family_domain_origins: usize,
    pub max_family_domain_polynomial_terms: usize,
    pub max_family_domain_exponent_entries: usize,
    pub max_family_manifest_bytes: usize,
    pub max_retained_coefficient_bytes: usize,
}

impl Default for GenericTensorProjectorLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            // Rank eight has 105 pairings.  Higher ranks should use an
            // orbit-reduced projector rather than a dense generic inverse.
            max_rank: 8,
            max_pairings: 105,
            max_input_vectors: 4_096,
            max_input_spectator_vectors: 4_096,
            max_input_metrics: 4_096,
            max_input_scalar_product_factors: 4_096,
            max_input_spectator_scalar_product_factors: 4_096,
            max_scalar_product_degree: u64::from(u16::MAX),
            max_spectator_scalar_product_degree: u64::from(u16::MAX),
            max_index_endpoints: 16_384,
            max_gram_entries: 1_000_000,
            max_augmented_entries: 2_000_000,
            max_matrix_live_entries: 4_000_000,
            max_matrix_input_retained_bytes: 1024 * 1024 * 1024,
            max_matrix_output_retained_bytes: 1024 * 1024 * 1024,
            max_projection_candidates: 1_000_000,
            max_output_terms: 1_000_000,
            max_output_structure_entries: 64_000_000,
            max_arithmetic_operations: 32_000_000,
            max_structural_operations: 32_000_000,
            max_nonzero_conditions: 1_000_000,
            max_guard_origins_per_condition: 1_000_000,
            max_guard_origins: 100_000_000,
            max_guard_polynomial_terms: 16_000_000,
            max_guard_exponent_entries: 64_000_000,
            max_family_domain_conditions: 1_000_000,
            max_family_domain_origins: 100_000_000,
            max_family_domain_polynomial_terms: 16_000_000,
            max_family_domain_exponent_entries: 64_000_000,
            max_family_manifest_bytes: 64 * 1024 * 1024,
            max_retained_coefficient_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Where a loop-vector id occurred in the original tensor monomial.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TensorLoopReference {
    IndexedVector {
        position: usize,
        index: LorentzIndex,
    },
    ScalarProductLeft {
        left: LoopVector,
        right: LoopVector,
    },
    ScalarProductRight {
        left: LoopVector,
        right: LoopVector,
    },
}

/// Stable id of a Lorentz vector that is external to the integration but is
/// not an external momentum of the integral family.
///
/// These are the `p(...)`/`vec1(...)` spectators in Vakint tensor numerators.
/// They survive the loop integration as covariants and must not be added to an
/// `IntegralFamily`'s affine scalar-product basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpectatorVector(u32);

impl SpectatorVector {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u32 {
        self.0
    }
}

impl From<u32> for SpectatorVector {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

/// One spectator vector carrying a Lorentz index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexedSpectatorVector {
    vector: SpectatorVector,
    index: LorentzIndex,
}

impl IndexedSpectatorVector {
    pub const fn new(vector: SpectatorVector, index: LorentzIndex) -> Self {
        Self { vector, index }
    }

    pub const fn vector(self) -> SpectatorVector {
        self.vector
    }

    pub const fn index(self) -> LorentzIndex {
        self.index
    }
}

/// A symmetric scalar product of two spectator covariants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpectatorScalarProduct {
    left: SpectatorVector,
    right: SpectatorVector,
}

impl SpectatorScalarProduct {
    pub fn new(left: SpectatorVector, right: SpectatorVector) -> Self {
        if left <= right {
            Self { left, right }
        } else {
            Self {
                left: right,
                right: left,
            }
        }
    }

    pub const fn left(self) -> SpectatorVector {
        self.left
    }

    pub const fn right(self) -> SpectatorVector {
        self.right
    }
}

/// Canonical commutative monomial in spectator scalar products.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpectatorScalarProductMonomial {
    factors: BTreeMap<SpectatorScalarProduct, u32>,
}

impl SpectatorScalarProductMonomial {
    pub fn one() -> Self {
        Self::default()
    }

    pub fn try_from_factors_with_limits(
        factors: impl IntoIterator<Item = (SpectatorScalarProduct, u32)>,
        limits: GenericTensorProjectorLimits,
    ) -> Result<Self, GenericTensorProjectorError> {
        let mut result = Self::one();
        let mut entries = 0usize;
        for (factor, exponent) in factors {
            entries = entries.checked_add(1).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "spectator scalar-product constructor entries",
                },
            )?;
            check_usize_limit(
                "spectator scalar-product constructor entries",
                entries,
                limits.max_input_spectator_scalar_product_factors,
            )?;
            result.try_multiply_power(factor, exponent)?;
            let degree = result.checked_degree()?;
            if degree > limits.max_spectator_scalar_product_degree {
                return Err(
                    GenericTensorProjectorError::SpectatorScalarProductDegreeLimit {
                        requested: degree,
                        limit: limits.max_spectator_scalar_product_degree,
                    },
                );
            }
        }
        Ok(result)
    }

    pub fn factors(&self) -> &BTreeMap<SpectatorScalarProduct, u32> {
        &self.factors
    }

    pub fn exponent(&self, factor: SpectatorScalarProduct) -> u32 {
        self.factors.get(&factor).copied().unwrap_or(0)
    }

    pub fn checked_degree(&self) -> Result<u64, GenericTensorProjectorError> {
        self.factors.values().try_fold(0u64, |degree, &exponent| {
            degree.checked_add(u64::from(exponent)).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "spectator scalar-product degree",
                },
            )
        })
    }

    pub fn is_one(&self) -> bool {
        self.factors.is_empty()
    }

    pub fn try_multiply_power(
        &mut self,
        factor: SpectatorScalarProduct,
        exponent: u32,
    ) -> Result<(), GenericTensorProjectorError> {
        if exponent == 0 {
            return Ok(());
        }
        let current = self.factors.entry(factor).or_default();
        *current = current.checked_add(exponent).ok_or(
            GenericTensorProjectorError::SpectatorScalarProductExponentOverflow { factor },
        )?;
        Ok(())
    }
}

/// Original tensor numerator with loop vectors and spectator covariants kept
/// as distinct typed objects.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CovariantTensorMonomial {
    loop_vectors: Vec<IndexedVector>,
    spectator_vectors: Vec<IndexedSpectatorVector>,
    metrics: Vec<Metric>,
    loop_scalar_products: crate::ScalarProductMonomial,
    spectator_scalar_products: SpectatorScalarProductMonomial,
}

impl CovariantTensorMonomial {
    pub fn try_from_parts_with_limits(
        loop_vectors: impl IntoIterator<Item = IndexedVector>,
        spectator_vectors: impl IntoIterator<Item = IndexedSpectatorVector>,
        metrics: impl IntoIterator<Item = Metric>,
        loop_scalar_products: crate::ScalarProductMonomial,
        spectator_scalar_products: SpectatorScalarProductMonomial,
        limits: GenericTensorProjectorLimits,
    ) -> Result<Self, GenericTensorProjectorError> {
        let loop_vectors = bounded_collect(
            loop_vectors,
            "covariant tensor input loop vectors",
            limits.max_input_vectors,
        )?;
        let spectator_vectors = bounded_collect(
            spectator_vectors,
            "covariant tensor input spectator vectors",
            limits.max_input_spectator_vectors,
        )?;
        let metrics = bounded_collect(
            metrics,
            "covariant tensor input metrics",
            limits.max_input_metrics,
        )?;
        Ok(Self {
            loop_vectors,
            spectator_vectors,
            metrics,
            loop_scalar_products,
            spectator_scalar_products,
        })
    }

    pub fn from_loop_tensor_with_limits(
        source: &TensorMonomial,
        limits: GenericTensorProjectorLimits,
    ) -> Result<Self, GenericTensorProjectorError> {
        Self::try_from_parts_with_limits(
            source.vectors().iter().copied(),
            [],
            source.metrics().iter().copied(),
            source.scalar_products().clone(),
            SpectatorScalarProductMonomial::one(),
            limits,
        )
    }

    pub fn loop_vectors(&self) -> &[IndexedVector] {
        &self.loop_vectors
    }

    pub fn spectator_vectors(&self) -> &[IndexedSpectatorVector] {
        &self.spectator_vectors
    }

    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }

    pub const fn loop_scalar_products(&self) -> &crate::ScalarProductMonomial {
        &self.loop_scalar_products
    }

    pub const fn spectator_scalar_products(&self) -> &SpectatorScalarProductMonomial {
        &self.spectator_scalar_products
    }
}

/// Canonical Lorentz covariant multiplying a scalar integral.
///
/// This complete object, rather than only its metric pairing, is the key a
/// tensor/IBP composition layer must use.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct TensorCovariantStructure {
    metrics: MetricPairing,
    spectator_vectors: Vec<IndexedSpectatorVector>,
    spectator_scalar_products: SpectatorScalarProductMonomial,
}

impl TensorCovariantStructure {
    pub fn new(
        metrics: MetricPairing,
        mut spectator_vectors: Vec<IndexedSpectatorVector>,
        spectator_scalar_products: SpectatorScalarProductMonomial,
    ) -> Self {
        spectator_vectors.sort_unstable();
        Self {
            metrics,
            spectator_vectors,
            spectator_scalar_products,
        }
    }

    pub const fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub fn spectator_vectors(&self) -> &[IndexedSpectatorVector] {
        &self.spectator_vectors
    }

    pub const fn spectator_scalar_products(&self) -> &SpectatorScalarProductMonomial {
        &self.spectator_scalar_products
    }

    pub fn is_metric_only(&self) -> bool {
        self.spectator_vectors.is_empty() && self.spectator_scalar_products.is_one()
    }
}

/// One exact projected covariant term.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericCovariantTensorTerm {
    coefficient: Coefficient,
    covariant: TensorCovariantStructure,
    loop_scalar_products: crate::GenericScalarProductMonomial,
}

impl GenericCovariantTensorTerm {
    pub fn new(
        coefficient: Coefficient,
        covariant: TensorCovariantStructure,
        loop_scalar_products: crate::GenericScalarProductMonomial,
    ) -> Self {
        Self {
            coefficient,
            covariant,
            loop_scalar_products,
        }
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub const fn covariant(&self) -> &TensorCovariantStructure {
        &self.covariant
    }

    pub const fn loop_scalar_products(&self) -> &crate::GenericScalarProductMonomial {
        &self.loop_scalar_products
    }
}

/// Sparse projected numerator keyed by the complete Lorentz covariant.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenericCovariantTensorNumerator {
    terms: Vec<GenericCovariantTensorTerm>,
}

impl GenericCovariantTensorNumerator {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn try_new_with_limit(
        terms: impl IntoIterator<Item = GenericCovariantTensorTerm>,
        max_terms: usize,
    ) -> Result<Self, GenericTensorProjectorError> {
        Ok(Self {
            terms: bounded_collect(terms, "covariant tensor output terms", max_terms)?,
        })
    }

    pub fn terms(&self) -> &[GenericCovariantTensorTerm] {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Exact adapter to the metric-only generic family bridge.
    pub fn try_metric_numerator(
        &self,
        max_terms: usize,
    ) -> Result<GenericTensorNumerator, GenericTensorProjectorError> {
        let mut terms = Vec::new();
        for (term, value) in self.terms.iter().enumerate() {
            if !value.covariant.is_metric_only() {
                return Err(
                    GenericTensorProjectorError::SpectatorCovariantCannotUseMetricBridge { term },
                );
            }
            check_usize_limit(
                "metric-only tensor adapter terms",
                terms.len().saturating_add(1),
                max_terms,
            )?;
            terms.push(GenericTensorTerm::new(
                value.coefficient.clone(),
                value.covariant.metrics.clone(),
                value.loop_scalar_products.clone(),
            ));
        }
        Ok(GenericTensorNumerator::try_new_with_limit(
            terms, max_terms,
        )?)
    }
}

/// Typed provenance for every extra nonzero condition introduced by tensor
/// projection.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TensorProjectionGuardOrigin {
    MetricContractionCoefficientDenominator,
    GramEntryDenominator {
        row: usize,
        column: usize,
    },
    /// Legacy V1 provenance retained for schema readers. V2 projectors emit
    /// [`Self::ProjectorGramDeterminantNumerator`] instead.
    ProjectorPivotNumerator {
        rank: usize,
        column: usize,
    },
    /// Legacy V1 provenance retained for schema readers. V2 projectors emit
    /// [`Self::ProjectorGramDeterminantDenominator`] instead.
    ProjectorPivotDenominator {
        rank: usize,
        column: usize,
    },
    ProjectorGramDeterminantNumerator {
        rank: usize,
    },
    ProjectorGramDeterminantDenominator {
        rank: usize,
    },
    InverseGramDenominator {
        row: usize,
        column: usize,
    },
    ProjectedCoefficientDenominator {
        output_term: usize,
    },
}

/// One normalized polynomial condition and all projection steps requiring it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorProjectionNonZeroCondition {
    polynomial: BasePolynomial,
    origins: BTreeSet<TensorProjectionGuardOrigin>,
}

impl TensorProjectionNonZeroCondition {
    pub fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<TensorProjectionGuardOrigin> {
        &self.origins
    }
}

/// Complete exceptional domain of an authenticated tensor projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericTensorProjectionDomain {
    family: FamilyDomain,
    projection_nonzero: Vec<TensorProjectionNonZeroCondition>,
}

impl GenericTensorProjectionDomain {
    pub const fn family(&self) -> &FamilyDomain {
        &self.family
    }

    pub fn projection_nonzero_conditions(&self) -> &[TensorProjectionNonZeroCondition] {
        &self.projection_nonzero
    }
}

/// Exact metric-contraction state retained before the O(d) projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VacuumMetricContractionWitness {
    closed_metric_loops: u64,
    coefficient: Coefficient,
    vectors: Vec<IndexedVector>,
    metrics: MetricPairing,
    scalar_products: crate::GenericScalarProductMonomial,
}

impl VacuumMetricContractionWitness {
    pub const fn closed_metric_loops(&self) -> u64 {
        self.closed_metric_loops
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn vectors(&self) -> &[IndexedVector] {
        &self.vectors
    }

    pub const fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub const fn scalar_products(&self) -> &crate::GenericScalarProductMonomial {
        &self.scalar_products
    }
}

/// Replayable exact witness for the dense vacuum projector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VacuumTensorProjectionWitness {
    contraction: VacuumMetricContractionWitness,
    rank: usize,
    pairings: Vec<SlotPairing>,
    inverse_gram: Vec<Vec<Coefficient>>,
}

impl VacuumTensorProjectionWitness {
    pub const fn contraction(&self) -> &VacuumMetricContractionWitness {
        &self.contraction
    }

    pub const fn rank(&self) -> usize {
        self.rank
    }

    pub fn pairings(&self) -> &[SlotPairing] {
        &self.pairings
    }

    pub fn inverse_gram(&self) -> &[Vec<Coefficient>] {
        &self.inverse_gram
    }
}

/// Auditable aggregate work and retained-data counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenericTensorProjectionStats {
    pub arithmetic_operations: u64,
    /// Subset of `arithmetic_operations` performed inside authenticated
    /// Symbolica coefficient-algebra sessions.
    pub symbolica_algebra_operations: u64,
    pub structural_operations: u64,
    pub pairing_count: usize,
    pub gram_entries: usize,
    pub inverse_entries: usize,
    pub projection_candidates: usize,
    pub output_terms: usize,
    pub consumed_output_structure_entries: usize,
    pub guard_polynomial_terms: usize,
    pub guard_exponent_entries: usize,
    pub guard_origins: usize,
    pub family_domain_origins: usize,
    pub retained_coefficient_bytes: usize,
    /// Aggregate clone-owned caller inputs copied into Symbolica sessions.
    pub matrix_input_retained_bytes: usize,
    /// Aggregate clone-owned Symbolica outputs authenticated during projection.
    pub matrix_output_retained_bytes: usize,
    /// Largest conservative simultaneously-live Symbolica matrix payload
    /// admitted by any algebra session in this projection.
    pub matrix_peak_live_entries: usize,
}

/// The complete authenticated output of projecting one original monomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumTensorProjection {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    loop_order: Vec<String>,
    dimension: Coefficient,
    source: TensorMonomial,
    limits: GenericTensorProjectorLimits,
    domain: GenericTensorProjectionDomain,
    witness: VacuumTensorProjectionWitness,
    numerator: GenericTensorNumerator,
    stats: GenericTensorProjectionStats,
}

/// Proof-preserving composition of an authenticated projection with generic
/// scalar-product-to-integral lowering.
///
/// Consumers must retain this object (or both of its children), not extract
/// only `lowering`: projector determinant guards and the original tensor witness live
/// in `projection`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumTensorLowering {
    schema: &'static str,
    projection: AuthenticatedVacuumTensorProjection,
    lowering_limits: GenericTensorFamilyLimits,
    lowering: GenericTensorIntegralReduction,
}

impl AuthenticatedVacuumTensorLowering {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn projection(&self) -> &AuthenticatedVacuumTensorProjection {
        &self.projection
    }

    pub const fn lowering_limits(&self) -> GenericTensorFamilyLimits {
        self.lowering_limits
    }

    pub const fn lowering(&self) -> &GenericTensorIntegralReduction {
        &self.lowering
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorProjectorError> {
        self.projection.verify(family)?;
        let replay = GenericTensorFamilyReducer::with_limits(family, self.lowering_limits)
            .lower(self.lowering.base_integral(), self.projection.numerator())?;
        if replay == self.lowering {
            Ok(())
        } else {
            Err(GenericTensorProjectorError::InternalVerificationFailure {
                detail: "replayed scalar lowering differs from the projection-bound lowering"
                    .to_owned(),
            })
        }
    }
}

/// Exact pre-projection state for a spectator-covariant numerator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum VacuumCovariantVectorContractionWitness {
    LoopLoop {
        index: LorentzIndex,
        left: LoopVector,
        right: LoopVector,
    },
    SpectatorSpectator {
        index: LorentzIndex,
        left: SpectatorVector,
        right: SpectatorVector,
    },
}

/// Exact pre-projection state for a spectator-covariant numerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VacuumCovariantPrecontractionWitness {
    closed_metric_loops: u64,
    coefficient: Coefficient,
    vector_contractions: Vec<VacuumCovariantVectorContractionWitness>,
    loop_vectors: Vec<IndexedVector>,
    spectator_vectors: Vec<IndexedSpectatorVector>,
    metrics: MetricPairing,
    loop_scalar_products: crate::GenericScalarProductMonomial,
    spectator_scalar_products: SpectatorScalarProductMonomial,
}

impl VacuumCovariantPrecontractionWitness {
    pub const fn closed_metric_loops(&self) -> u64 {
        self.closed_metric_loops
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    /// Vakint-compatible same-index vector contractions performed before
    /// wiring any metric endpoints. This order makes expressions such as
    /// `(k_mu k_nu)^2 g^{mu nu}` unambiguous: each equal-index vector pair is
    /// converted to a scalar product and the metric remains an outside
    /// covariant.
    pub fn vector_contractions(&self) -> &[VacuumCovariantVectorContractionWitness] {
        &self.vector_contractions
    }

    pub fn loop_vectors(&self) -> &[IndexedVector] {
        &self.loop_vectors
    }

    pub fn spectator_vectors(&self) -> &[IndexedSpectatorVector] {
        &self.spectator_vectors
    }

    pub const fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub const fn loop_scalar_products(&self) -> &crate::GenericScalarProductMonomial {
        &self.loop_scalar_products
    }

    pub const fn spectator_scalar_products(&self) -> &SpectatorScalarProductMonomial {
        &self.spectator_scalar_products
    }
}

/// Replayable witness for spectator-covariant projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VacuumCovariantTensorProjectionWitness {
    precontraction: VacuumCovariantPrecontractionWitness,
    rank: usize,
    pairings: Vec<SlotPairing>,
    inverse_gram: Vec<Vec<Coefficient>>,
}

impl VacuumCovariantTensorProjectionWitness {
    pub const fn precontraction(&self) -> &VacuumCovariantPrecontractionWitness {
        &self.precontraction
    }

    pub const fn rank(&self) -> usize {
        self.rank
    }

    pub fn pairings(&self) -> &[SlotPairing] {
        &self.pairings
    }

    pub fn inverse_gram(&self) -> &[Vec<Coefficient>] {
        &self.inverse_gram
    }
}

/// Authenticated projection retaining spectator vectors as genuine Lorentz
/// covariants while the integral family remains a vacuum family (`E=0`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorProjection {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    loop_order: Vec<String>,
    dimension: Coefficient,
    source: CovariantTensorMonomial,
    limits: GenericTensorProjectorLimits,
    domain: GenericTensorProjectionDomain,
    witness: VacuumCovariantTensorProjectionWitness,
    numerator: GenericCovariantTensorNumerator,
    stats: GenericTensorProjectionStats,
}

impl AuthenticatedVacuumCovariantTensorProjection {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn loop_order(&self) -> &[String] {
        &self.loop_order
    }

    pub const fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub const fn source(&self) -> &CovariantTensorMonomial {
        &self.source
    }

    pub const fn limits(&self) -> GenericTensorProjectorLimits {
        self.limits
    }

    pub const fn domain(&self) -> &GenericTensorProjectionDomain {
        &self.domain
    }

    pub const fn witness(&self) -> &VacuumCovariantTensorProjectionWitness {
        &self.witness
    }

    pub const fn numerator(&self) -> &GenericCovariantTensorNumerator {
        &self.numerator
    }

    pub const fn stats(&self) -> GenericTensorProjectionStats {
        self.stats
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorProjectorError> {
        let actual: Arc<str> = Arc::from(family.fingerprint());
        if actual != self.family_fingerprint {
            return Err(GenericTensorProjectorError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual,
            });
        }
        let replay = GenericVacuumTensorProjector::with_limits(self.limits)
            .project_covariant(family, &self.source)?;
        if replay == *self {
            Ok(())
        } else {
            Err(GenericTensorProjectorError::InternalVerificationFailure {
                detail: "replayed spectator-covariant projection differs from the retained result"
                    .to_owned(),
            })
        }
    }
}

impl AuthenticatedVacuumTensorProjection {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Ordered family labels authenticated by zero-based [`LoopVector`] ids.
    pub fn loop_order(&self) -> &[String] {
        &self.loop_order
    }

    /// The exact dimension copied from `IntegralFamily::dimension()`.
    pub const fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub const fn source(&self) -> &TensorMonomial {
        &self.source
    }

    pub const fn limits(&self) -> GenericTensorProjectorLimits {
        self.limits
    }

    pub const fn domain(&self) -> &GenericTensorProjectionDomain {
        &self.domain
    }

    pub const fn witness(&self) -> &VacuumTensorProjectionWitness {
        &self.witness
    }

    pub const fn numerator(&self) -> &GenericTensorNumerator {
        &self.numerator
    }

    pub const fn stats(&self) -> GenericTensorProjectionStats {
        self.stats
    }

    /// Lower the projected loop scalar products while retaining the complete
    /// projection proof and exceptional domain.
    pub fn lower(
        &self,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
    ) -> Result<AuthenticatedVacuumTensorLowering, GenericTensorProjectorError> {
        self.lower_with_limits(family, base_integral, GenericTensorFamilyLimits::default())
    }

    pub fn lower_with_limits(
        &self,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
        limits: GenericTensorFamilyLimits,
    ) -> Result<AuthenticatedVacuumTensorLowering, GenericTensorProjectorError> {
        self.verify(family)?;
        let lowering = GenericTensorFamilyReducer::with_limits(family, limits)
            .lower(base_integral, &self.numerator)?;
        Ok(AuthenticatedVacuumTensorLowering {
            schema: AUTHENTICATED_VACUUM_TENSOR_LOWERING_V2_SCHEMA,
            projection: self.clone(),
            lowering_limits: limits,
            lowering,
        })
    }

    /// Re-run the complete projection against the supplied family.
    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorProjectorError> {
        let actual: Arc<str> = Arc::from(family.fingerprint());
        if actual != self.family_fingerprint {
            return Err(GenericTensorProjectorError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual,
            });
        }
        let replay =
            GenericVacuumTensorProjector::with_limits(self.limits).project(family, &self.source)?;
        if replay == *self {
            Ok(())
        } else {
            Err(GenericTensorProjectorError::InternalVerificationFailure {
                detail: "replayed vacuum tensor projection differs from the retained result"
                    .to_owned(),
            })
        }
    }
}

/// Stateless authenticated projector.  A future orbit-reduced implementation
/// can preserve this API and witness format while replacing the dense solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericVacuumTensorProjector {
    limits: GenericTensorProjectorLimits,
}

impl Default for GenericVacuumTensorProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericVacuumTensorProjector {
    pub fn new() -> Self {
        Self::with_limits(GenericTensorProjectorLimits::default())
    }

    pub const fn with_limits(limits: GenericTensorProjectorLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(&self) -> GenericTensorProjectorLimits {
        self.limits
    }

    /// Project one original tensor monomial using exactly the family dimension.
    pub fn project(
        &self,
        family: &IntegralFamily,
        source: &TensorMonomial,
    ) -> Result<AuthenticatedVacuumTensorProjection, GenericTensorProjectorError> {
        if family.external_count() != 0 {
            return Err(GenericTensorProjectorError::ExternalMomentaUnsupported {
                externals: family.external_count(),
            });
        }

        let context = family.coefficient_context();
        context.validate_with_limits(family.dimension(), self.limits.exact_algebra)?;
        let family_domain_origins = self.validate_family_retention(family)?;
        self.validate_source(family, source)?;

        let fingerprint = family.fingerprint();
        let manifest_bytes = manifest_bytes(family, &fingerprint)?;
        check_usize_limit(
            "tensor projector family manifest bytes",
            manifest_bytes,
            self.limits.max_family_manifest_bytes,
        )?;

        let mut budget = ProjectionBudget::default();
        charge_coefficient_bytes(
            family.dimension(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;
        charge_coefficient_bytes(
            family.domain().basis_determinant(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;

        let contraction = self.contract_metrics(family, source, &mut budget)?;
        charge_coefficient_bytes(
            contraction.coefficient(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;

        let mut guards = ProjectionGuardBuilder::default();
        guards.insert(
            contraction.coefficient().denominator.clone(),
            TensorProjectionGuardOrigin::MetricContractionCoefficientDenominator,
            self.limits,
            &mut budget,
        )?;

        let rank = contraction.vectors.len();
        check_usize_limit("tensor projector rank", rank, self.limits.max_rank)?;

        let (pairings, inverse_gram, numerator, projection_candidates) = if rank % 2 == 1 {
            (Vec::new(), Vec::new(), GenericTensorNumerator::zero(), 0)
        } else if rank == 0 {
            let term = GenericTensorTerm::new(
                contraction.coefficient.clone(),
                contraction.metrics.clone(),
                contraction.scalar_products.clone(),
            );
            guards.insert(
                term.coefficient().denominator.clone(),
                TensorProjectionGuardOrigin::ProjectedCoefficientDenominator { output_term: 0 },
                self.limits,
                &mut budget,
            )?;
            charge_coefficient_bytes(
                term.coefficient(),
                &mut budget,
                self.limits.max_retained_coefficient_bytes,
            )?;
            (
                Vec::new(),
                Vec::new(),
                GenericTensorNumerator::try_new_with_limit([term], self.limits.max_output_terms)?,
                0,
            )
        } else {
            self.project_even_rank(family, &contraction, &mut guards, &mut budget)?
        };

        let stats = GenericTensorProjectionStats {
            arithmetic_operations: budget.arithmetic_operations,
            symbolica_algebra_operations: budget.symbolica_algebra_operations,
            structural_operations: budget.structural_operations,
            pairing_count: pairings.len(),
            gram_entries: pairings.len().checked_mul(pairings.len()).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector Gram entries",
                },
            )?,
            inverse_entries: inverse_gram.iter().map(Vec::len).sum(),
            projection_candidates,
            output_terms: numerator.terms().len(),
            consumed_output_structure_entries: budget.consumed_output_structure_entries,
            guard_polynomial_terms: budget.guard_polynomial_terms,
            guard_exponent_entries: budget.guard_exponent_entries,
            guard_origins: budget.guard_origins,
            family_domain_origins,
            retained_coefficient_bytes: budget.retained_coefficient_bytes,
            matrix_input_retained_bytes: budget.matrix_input_retained_bytes,
            matrix_output_retained_bytes: budget.matrix_output_retained_bytes,
            matrix_peak_live_entries: budget.matrix_peak_live_entries,
        };

        Ok(AuthenticatedVacuumTensorProjection {
            schema: GENERIC_VACUUM_TENSOR_PROJECTION_V2_SCHEMA,
            family_fingerprint: Arc::from(fingerprint),
            loop_order: family.loop_momenta().to_vec(),
            dimension: family.dimension().clone(),
            source: source.clone(),
            limits: self.limits,
            domain: GenericTensorProjectionDomain {
                family: family.domain().clone(),
                projection_nonzero: guards.conditions,
            },
            witness: VacuumTensorProjectionWitness {
                contraction,
                rank,
                pairings,
                inverse_gram,
            },
            numerator,
            stats,
        })
    }

    /// Project loop tensors while retaining numerator-only external vectors as
    /// canonical spectator covariants.  The supplied integral family must
    /// still be a vacuum family; spectators never enter its denominator basis.
    pub fn project_covariant(
        &self,
        family: &IntegralFamily,
        source: &CovariantTensorMonomial,
    ) -> Result<AuthenticatedVacuumCovariantTensorProjection, GenericTensorProjectorError> {
        if family.external_count() != 0 {
            return Err(GenericTensorProjectorError::ExternalMomentaUnsupported {
                externals: family.external_count(),
            });
        }
        let context = family.coefficient_context();
        context.validate_with_limits(family.dimension(), self.limits.exact_algebra)?;
        let family_domain_origins = self.validate_family_retention(family)?;
        self.validate_covariant_source(family, source)?;

        let fingerprint = family.fingerprint();
        let manifest_bytes = manifest_bytes(family, &fingerprint)?;
        check_usize_limit(
            "tensor projector family manifest bytes",
            manifest_bytes,
            self.limits.max_family_manifest_bytes,
        )?;

        let mut budget = ProjectionBudget::default();
        charge_coefficient_bytes(
            family.dimension(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;
        charge_coefficient_bytes(
            family.domain().basis_determinant(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;
        let precontraction = self.precontract_covariants(family, source, &mut budget)?;
        charge_coefficient_bytes(
            precontraction.coefficient(),
            &mut budget,
            self.limits.max_retained_coefficient_bytes,
        )?;

        let mut guards = ProjectionGuardBuilder::default();
        guards.insert(
            precontraction.coefficient().denominator.clone(),
            TensorProjectionGuardOrigin::MetricContractionCoefficientDenominator,
            self.limits,
            &mut budget,
        )?;
        let rank = precontraction.loop_vectors.len();
        check_usize_limit("tensor projector rank", rank, self.limits.max_rank)?;

        let (pairings, inverse_gram, numerator, projection_candidates) = if rank % 2 == 1 {
            (
                Vec::new(),
                Vec::new(),
                GenericCovariantTensorNumerator::zero(),
                0,
            )
        } else if rank == 0 {
            let (post_factor, covariant) = self.contract_spectator_output(
                family,
                precontraction.metrics.metrics(),
                &precontraction.spectator_vectors,
                &precontraction.spectator_scalar_products,
                &mut budget,
            )?;
            let coefficient = checked_mul(
                context,
                &precontraction.coefficient,
                &post_factor,
                self.limits,
                &mut budget,
            )?;
            let term = GenericCovariantTensorTerm::new(
                coefficient,
                covariant,
                precontraction.loop_scalar_products.clone(),
            );
            guards.insert(
                term.coefficient().denominator.clone(),
                TensorProjectionGuardOrigin::ProjectedCoefficientDenominator { output_term: 0 },
                self.limits,
                &mut budget,
            )?;
            charge_coefficient_bytes(
                term.coefficient(),
                &mut budget,
                self.limits.max_retained_coefficient_bytes,
            )?;
            (
                Vec::new(),
                Vec::new(),
                GenericCovariantTensorNumerator::try_new_with_limit(
                    [term],
                    self.limits.max_output_terms,
                )?,
                0,
            )
        } else {
            self.project_covariant_even_rank(family, &precontraction, &mut guards, &mut budget)?
        };

        let gram_entries = pairings.len().checked_mul(pairings.len()).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector Gram entries",
            },
        )?;
        let stats = GenericTensorProjectionStats {
            arithmetic_operations: budget.arithmetic_operations,
            symbolica_algebra_operations: budget.symbolica_algebra_operations,
            structural_operations: budget.structural_operations,
            pairing_count: pairings.len(),
            gram_entries,
            inverse_entries: inverse_gram.iter().map(Vec::len).sum(),
            projection_candidates,
            output_terms: numerator.terms().len(),
            consumed_output_structure_entries: budget.consumed_output_structure_entries,
            guard_polynomial_terms: budget.guard_polynomial_terms,
            guard_exponent_entries: budget.guard_exponent_entries,
            guard_origins: budget.guard_origins,
            family_domain_origins,
            retained_coefficient_bytes: budget.retained_coefficient_bytes,
            matrix_input_retained_bytes: budget.matrix_input_retained_bytes,
            matrix_output_retained_bytes: budget.matrix_output_retained_bytes,
            matrix_peak_live_entries: budget.matrix_peak_live_entries,
        };

        Ok(AuthenticatedVacuumCovariantTensorProjection {
            schema: GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V2_SCHEMA,
            family_fingerprint: Arc::from(fingerprint),
            loop_order: family.loop_momenta().to_vec(),
            dimension: family.dimension().clone(),
            source: source.clone(),
            limits: self.limits,
            domain: GenericTensorProjectionDomain {
                family: family.domain().clone(),
                projection_nonzero: guards.conditions,
            },
            witness: VacuumCovariantTensorProjectionWitness {
                precontraction,
                rank,
                pairings,
                inverse_gram,
            },
            numerator,
            stats,
        })
    }

    fn validate_covariant_source(
        &self,
        family: &IntegralFamily,
        source: &CovariantTensorMonomial,
    ) -> Result<(), GenericTensorProjectorError> {
        check_usize_limit(
            "covariant tensor input loop vectors",
            source.loop_vectors.len(),
            self.limits.max_input_vectors,
        )?;
        check_usize_limit(
            "covariant tensor input spectator vectors",
            source.spectator_vectors.len(),
            self.limits.max_input_spectator_vectors,
        )?;
        check_usize_limit(
            "covariant tensor input metrics",
            source.metrics.len(),
            self.limits.max_input_metrics,
        )?;
        check_usize_limit(
            "covariant tensor input loop scalar-product factors",
            source.loop_scalar_products.factors().len(),
            self.limits.max_input_scalar_product_factors,
        )?;
        check_usize_limit(
            "covariant tensor input spectator scalar-product factors",
            source.spectator_scalar_products.factors().len(),
            self.limits.max_input_spectator_scalar_product_factors,
        )?;
        let vector_endpoints = source
            .loop_vectors
            .len()
            .checked_add(source.spectator_vectors.len())
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "covariant tensor index endpoints",
            })?;
        let endpoints = vector_endpoints
            .checked_add(source.metrics.len().checked_mul(2).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "covariant tensor index endpoints",
                },
            )?)
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "covariant tensor index endpoints",
            })?;
        check_usize_limit(
            "covariant tensor index endpoints",
            endpoints,
            self.limits.max_index_endpoints,
        )?;
        for (position, vector) in source.loop_vectors.iter().copied().enumerate() {
            validate_loop_vector(
                vector.vector(),
                family.loop_count(),
                TensorLoopReference::IndexedVector {
                    position,
                    index: vector.index(),
                },
            )?;
        }
        let mut loop_degree = 0u64;
        for (&factor, &exponent) in source.loop_scalar_products.factors() {
            validate_loop_vector(
                factor.left(),
                family.loop_count(),
                TensorLoopReference::ScalarProductLeft {
                    left: factor.left(),
                    right: factor.right(),
                },
            )?;
            validate_loop_vector(
                factor.right(),
                family.loop_count(),
                TensorLoopReference::ScalarProductRight {
                    left: factor.left(),
                    right: factor.right(),
                },
            )?;
            loop_degree = loop_degree.checked_add(u64::from(exponent)).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "covariant tensor loop scalar-product degree",
                },
            )?;
        }
        if loop_degree > self.limits.max_scalar_product_degree {
            return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                requested: loop_degree,
                limit: self.limits.max_scalar_product_degree,
            });
        }
        let spectator_degree = source.spectator_scalar_products.checked_degree()?;
        if spectator_degree > self.limits.max_spectator_scalar_product_degree {
            return Err(
                GenericTensorProjectorError::SpectatorScalarProductDegreeLimit {
                    requested: spectator_degree,
                    limit: self.limits.max_spectator_scalar_product_degree,
                },
            );
        }
        Ok(())
    }

    fn precontract_covariants(
        &self,
        family: &IntegralFamily,
        source: &CovariantTensorMonomial,
        budget: &mut ProjectionBudget,
    ) -> Result<VacuumCovariantPrecontractionWitness, GenericTensorProjectorError> {
        #[derive(Default)]
        struct SameIndexVectors {
            loops: Vec<LoopVector>,
            spectators: Vec<SpectatorVector>,
        }
        #[derive(Default)]
        struct IndexData {
            occurrences: usize,
            loops: Vec<LoopVector>,
            spectators: Vec<SpectatorVector>,
        }

        // Vakint contracts vec(p1,n)*vec(p2,n)=p1.p2 before outside metrics
        // are wired. Keep that ordering explicit in the witness. More than
        // two vector endpoints at one label has no unique pair selection and
        // is therefore rejected rather than guessed.
        let mut same_index = BTreeMap::<LorentzIndex, SameIndexVectors>::new();
        for vector in &source.loop_vectors {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            same_index
                .entry(vector.index())
                .or_default()
                .loops
                .push(vector.vector());
        }
        for vector in &source.spectator_vectors {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            same_index
                .entry(vector.index())
                .or_default()
                .spectators
                .push(vector.vector());
        }

        let mut loop_scalars = crate::GenericScalarProductMonomial::one();
        for (&factor, &exponent) in source.loop_scalar_products.factors() {
            loop_scalars.try_multiply_power(
                ScalarProductCoordinate::LoopLoop {
                    left: usize::from(factor.left().id()),
                    right: usize::from(factor.right().id()),
                },
                exponent,
            )?;
        }
        let mut spectator_scalars = source.spectator_scalar_products.clone();
        let mut vector_contractions = Vec::new();
        let mut index_data = BTreeMap::<LorentzIndex, IndexData>::new();
        for (index, mut vectors) in same_index {
            vectors.loops.sort_unstable();
            vectors.spectators.sort_unstable();
            let vector_count = vectors
                .loops
                .len()
                .checked_add(vectors.spectators.len())
                .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "same-index covariant vector endpoints",
                })?;
            if vector_count > 2 {
                return Err(
                    GenericTensorProjectorError::AmbiguousSameIndexVectorContraction {
                        index,
                        loop_vectors: vectors.loops.len(),
                        spectator_vectors: vectors.spectators.len(),
                    },
                );
            }
            match (vectors.loops.as_slice(), vectors.spectators.as_slice()) {
                (&[left, right], []) => {
                    loop_scalars.try_multiply_power(
                        ScalarProductCoordinate::LoopLoop {
                            left: usize::from(left.id()),
                            right: usize::from(right.id()),
                        },
                        1,
                    )?;
                    vector_contractions.push(VacuumCovariantVectorContractionWitness::LoopLoop {
                        index,
                        left,
                        right,
                    });
                }
                ([], &[left, right]) => {
                    spectator_scalars
                        .try_multiply_power(SpectatorScalarProduct::new(left, right), 1)?;
                    vector_contractions.push(
                        VacuumCovariantVectorContractionWitness::SpectatorSpectator {
                            index,
                            left,
                            right,
                        },
                    );
                }
                (loops, spectators) => {
                    let data = index_data.entry(index).or_default();
                    for &vector in loops {
                        data.occurrences = checked_occurrence(data.occurrences)?;
                        data.loops.push(vector);
                    }
                    for &vector in spectators {
                        data.occurrences = checked_occurrence(data.occurrences)?;
                        data.spectators.push(vector);
                    }
                }
            }
        }
        for metric in &source.metrics {
            for index in [metric.left(), metric.right()] {
                charge_structural(budget, 1, self.limits.max_structural_operations)?;
                let data = index_data.entry(index).or_default();
                data.occurrences = checked_occurrence(data.occurrences)?;
            }
        }
        for (&index, data) in &index_data {
            if data.occurrences > 2 {
                return Err(GenericTensorProjectorError::InvalidIndexMultiplicity {
                    index,
                    occurrences: data.occurrences,
                });
            }
        }
        let indices = index_data.keys().copied().collect::<Vec<_>>();
        let positions = indices
            .iter()
            .copied()
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect::<BTreeMap<_, _>>();
        let mut components = ProjectionDisjointSet::new(indices.len());
        for metric in &source.metrics {
            components.union(positions[&metric.left()], positions[&metric.right()]);
        }
        #[derive(Default)]
        struct Component {
            indices: Vec<LorentzIndex>,
            loops: Vec<LoopVector>,
            spectators: Vec<SpectatorVector>,
            free_indices: Vec<LorentzIndex>,
        }
        let mut grouped = BTreeMap::<usize, Component>::new();
        for (position, &index) in indices.iter().enumerate() {
            let data = &index_data[&index];
            let component = grouped.entry(components.find(position)).or_default();
            component.indices.push(index);
            component.loops.extend(data.loops.iter().copied());
            component.spectators.extend(data.spectators.iter().copied());
            if data.occurrences == 1 {
                component.free_indices.push(index);
            }
        }
        let mut closed_metric_loops = 0u64;
        let mut loop_vectors = Vec::new();
        let mut spectator_vectors = Vec::new();
        let mut metrics = Vec::new();
        for mut component in grouped.into_values() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            component.indices.sort_unstable();
            component.loops.sort_unstable();
            component.spectators.sort_unstable();
            component.free_indices.sort_unstable();
            match (
                component.loops.as_slice(),
                component.spectators.as_slice(),
                component.free_indices.as_slice(),
            ) {
                ([], [], []) => {
                    closed_metric_loops = closed_metric_loops.checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "covariant closed metric loops",
                        },
                    )?;
                }
                ([], [], &[left, right]) => metrics.push(Metric::new(left, right)),
                (&[loop_vector], [], &[index]) => {
                    loop_vectors.push(IndexedVector::new(loop_vector, index));
                }
                ([], &[spectator], &[index]) => {
                    spectator_vectors.push(IndexedSpectatorVector::new(spectator, index))
                }
                (&[left, right], [], []) => loop_scalars.try_multiply_power(
                    ScalarProductCoordinate::LoopLoop {
                        left: usize::from(left.id()),
                        right: usize::from(right.id()),
                    },
                    1,
                )?,
                ([], &[left, right], []) => spectator_scalars
                    .try_multiply_power(SpectatorScalarProduct::new(left, right), 1)?,
                (&[loop_vector], &[spectator], []) => {
                    let index = *component.indices.first().ok_or(
                        GenericTensorProjectorError::InternalVerificationFailure {
                            detail: "loop-spectator component has no Lorentz index".to_owned(),
                        },
                    )?;
                    loop_vectors.push(IndexedVector::new(loop_vector, index));
                    spectator_vectors.push(IndexedSpectatorVector::new(spectator, index));
                }
                (loops, spectators, free_indices) => {
                    return Err(
                        GenericTensorProjectorError::InvalidCovariantMetricComponent {
                            loop_vectors: loops.len(),
                            spectator_vectors: spectators.len(),
                            free_indices: free_indices.len(),
                        },
                    );
                }
            }
        }
        loop_vectors.sort_unstable();
        spectator_vectors.sort_unstable();
        let loop_degree = loop_scalars.checked_degree()?;
        if loop_degree > self.limits.max_scalar_product_degree {
            return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                requested: loop_degree,
                limit: self.limits.max_scalar_product_degree,
            });
        }
        let spectator_degree = spectator_scalars.checked_degree()?;
        if spectator_degree > self.limits.max_spectator_scalar_product_degree {
            return Err(
                GenericTensorProjectorError::SpectatorScalarProductDegreeLimit {
                    requested: spectator_degree,
                    limit: self.limits.max_spectator_scalar_product_degree,
                },
            );
        }
        let coefficient = checked_pow(
            family.coefficient_context(),
            family.dimension(),
            closed_metric_loops,
            self.limits,
            budget,
        )?;
        Ok(VacuumCovariantPrecontractionWitness {
            closed_metric_loops,
            coefficient,
            vector_contractions,
            loop_vectors,
            spectator_vectors,
            metrics: MetricPairing::new(metrics),
            loop_scalar_products: loop_scalars,
            spectator_scalar_products: spectator_scalars,
        })
    }

    fn project_covariant_even_rank(
        &self,
        family: &IntegralFamily,
        precontraction: &VacuumCovariantPrecontractionWitness,
        guards: &mut ProjectionGuardBuilder,
        budget: &mut ProjectionBudget,
    ) -> Result<
        (
            Vec<SlotPairing>,
            Vec<Vec<Coefficient>>,
            GenericCovariantTensorNumerator,
            usize,
        ),
        GenericTensorProjectorError,
    > {
        let rank = precontraction.loop_vectors.len();
        let (pairings, inverse_gram, gram_entries) =
            build_projector_data(family, rank, self.limits, guards, budget)?;
        let context = family.coefficient_context();
        let mut collected = BTreeMap::<
            (
                TensorCovariantStructure,
                crate::GenericScalarProductMonomial,
            ),
            Coefficient,
        >::new();
        for (output_position, output_pairing) in pairings.iter().enumerate() {
            let mut raw_metrics = precontraction.metrics.metrics().to_vec();
            raw_metrics.extend(output_pairing.pairs().iter().map(|&(left, right)| {
                Metric::new(
                    precontraction.loop_vectors[left].index(),
                    precontraction.loop_vectors[right].index(),
                )
            }));
            let (post_factor, covariant) = self.contract_spectator_output(
                family,
                &raw_metrics,
                &precontraction.spectator_vectors,
                &precontraction.spectator_scalar_products,
                budget,
            )?;
            for (source_position, source_pairing) in pairings.iter().enumerate() {
                charge_structural(budget, 1, self.limits.max_structural_operations)?;
                let mut loop_scalars = precontraction.loop_scalar_products.clone();
                for &(left, right) in source_pairing.pairs() {
                    loop_scalars.try_multiply_power(
                        ScalarProductCoordinate::LoopLoop {
                            left: usize::from(precontraction.loop_vectors[left].vector().id()),
                            right: usize::from(precontraction.loop_vectors[right].vector().id()),
                        },
                        1,
                    )?;
                }
                let degree = loop_scalars.checked_degree()?;
                if degree > self.limits.max_scalar_product_degree {
                    return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                        requested: degree,
                        limit: self.limits.max_scalar_product_degree,
                    });
                }
                let projector_coefficient = checked_mul(
                    context,
                    &precontraction.coefficient,
                    &inverse_gram[output_position][source_position],
                    self.limits,
                    budget,
                )?;
                let coefficient = checked_mul(
                    context,
                    &projector_coefficient,
                    &post_factor,
                    self.limits,
                    budget,
                )?;
                if coefficient.is_zero() {
                    continue;
                }
                let key = (covariant.clone(), loop_scalars);
                if let Some(current) = collected.get(&key) {
                    let sum = checked_add(context, current, &coefficient, self.limits, budget)?;
                    if sum.is_zero() {
                        collected.remove(&key);
                    } else {
                        collected.insert(key, sum);
                    }
                } else {
                    let attempted = collected.len().checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "covariant tensor projector output terms",
                        },
                    )?;
                    check_usize_limit(
                        "covariant tensor projector output terms",
                        attempted,
                        self.limits.max_output_terms,
                    )?;
                    let structure_entries = key
                        .0
                        .metrics
                        .metrics()
                        .len()
                        .checked_add(key.0.spectator_vectors.len())
                        .and_then(|entries| {
                            entries.checked_add(key.0.spectator_scalar_products.factors().len())
                        })
                        .and_then(|entries| entries.checked_add(key.1.factors().len()))
                        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "covariant tensor projector output structure entries",
                        })?;
                    budget.consumed_output_structure_entries = budget
                        .consumed_output_structure_entries
                        .checked_add(structure_entries)
                        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "covariant tensor projector output structure entries",
                        })?;
                    check_usize_limit(
                        "covariant tensor projector output structure entries",
                        budget.consumed_output_structure_entries,
                        self.limits.max_output_structure_entries,
                    )?;
                    collected.insert(key, coefficient);
                }
            }
        }
        let mut terms = Vec::with_capacity(collected.len());
        for ((covariant, loop_scalar_products), coefficient) in collected {
            let output_term = terms.len();
            guards.insert(
                coefficient.denominator.clone(),
                TensorProjectionGuardOrigin::ProjectedCoefficientDenominator { output_term },
                self.limits,
                budget,
            )?;
            charge_coefficient_bytes(
                &coefficient,
                budget,
                self.limits.max_retained_coefficient_bytes,
            )?;
            terms.push(GenericCovariantTensorTerm::new(
                coefficient,
                covariant,
                loop_scalar_products,
            ));
        }
        let numerator = GenericCovariantTensorNumerator::try_new_with_limit(
            terms,
            self.limits.max_output_terms,
        )?;
        Ok((pairings, inverse_gram, numerator, gram_entries))
    }

    fn contract_spectator_output(
        &self,
        family: &IntegralFamily,
        metrics: &[Metric],
        spectators: &[IndexedSpectatorVector],
        existing_scalars: &SpectatorScalarProductMonomial,
        budget: &mut ProjectionBudget,
    ) -> Result<(Coefficient, TensorCovariantStructure), GenericTensorProjectorError> {
        #[derive(Default)]
        struct IndexData {
            occurrences: usize,
            spectators: Vec<SpectatorVector>,
        }
        let mut index_data = BTreeMap::<LorentzIndex, IndexData>::new();
        for vector in spectators {
            let data = index_data.entry(vector.index()).or_default();
            data.occurrences = checked_occurrence(data.occurrences)?;
            data.spectators.push(vector.vector());
        }
        for metric in metrics {
            for index in [metric.left(), metric.right()] {
                let data = index_data.entry(index).or_default();
                data.occurrences = checked_occurrence(data.occurrences)?;
            }
        }
        for (&index, data) in &index_data {
            if data.occurrences > 2 {
                return Err(GenericTensorProjectorError::InvalidIndexMultiplicity {
                    index,
                    occurrences: data.occurrences,
                });
            }
        }
        let indices = index_data.keys().copied().collect::<Vec<_>>();
        let positions = indices
            .iter()
            .copied()
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect::<BTreeMap<_, _>>();
        let mut components = ProjectionDisjointSet::new(indices.len());
        for metric in metrics {
            components.union(positions[&metric.left()], positions[&metric.right()]);
        }
        #[derive(Default)]
        struct Component {
            spectators: Vec<SpectatorVector>,
            free_indices: Vec<LorentzIndex>,
        }
        let mut grouped = BTreeMap::<usize, Component>::new();
        for (position, &index) in indices.iter().enumerate() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            let data = &index_data[&index];
            let component = grouped.entry(components.find(position)).or_default();
            component.spectators.extend(data.spectators.iter().copied());
            if data.occurrences == 1 {
                component.free_indices.push(index);
            }
        }
        let mut closed_metric_loops = 0u64;
        let mut output_metrics = Vec::new();
        let mut output_spectators = Vec::new();
        let mut scalar_products = existing_scalars.clone();
        for mut component in grouped.into_values() {
            component.spectators.sort_unstable();
            component.free_indices.sort_unstable();
            match (
                component.spectators.as_slice(),
                component.free_indices.as_slice(),
            ) {
                ([], []) => {
                    closed_metric_loops = closed_metric_loops.checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "post-projection closed metric loops",
                        },
                    )?;
                }
                ([], &[left, right]) => output_metrics.push(Metric::new(left, right)),
                (&[spectator], &[index]) => {
                    output_spectators.push(IndexedSpectatorVector::new(spectator, index))
                }
                (&[left, right], []) => scalar_products
                    .try_multiply_power(SpectatorScalarProduct::new(left, right), 1)?,
                (spectators, free_indices) => {
                    return Err(
                        GenericTensorProjectorError::InvalidCovariantMetricComponent {
                            loop_vectors: 0,
                            spectator_vectors: spectators.len(),
                            free_indices: free_indices.len(),
                        },
                    );
                }
            }
        }
        let degree = scalar_products.checked_degree()?;
        if degree > self.limits.max_spectator_scalar_product_degree {
            return Err(
                GenericTensorProjectorError::SpectatorScalarProductDegreeLimit {
                    requested: degree,
                    limit: self.limits.max_spectator_scalar_product_degree,
                },
            );
        }
        let factor = checked_pow(
            family.coefficient_context(),
            family.dimension(),
            closed_metric_loops,
            self.limits,
            budget,
        )?;
        Ok((
            factor,
            TensorCovariantStructure::new(
                MetricPairing::new(output_metrics),
                output_spectators,
                scalar_products,
            ),
        ))
    }

    fn validate_family_retention(
        &self,
        family: &IntegralFamily,
    ) -> Result<usize, GenericTensorProjectorError> {
        let condition_count = family
            .domain()
            .input_denominators()
            .len()
            .checked_add(1)
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "retained tensor projector family-domain conditions",
            })?;
        check_usize_limit(
            "retained tensor projector family-domain conditions",
            condition_count,
            self.limits.max_family_domain_conditions,
        )?;
        let family_domain_origins = family
            .domain()
            .input_denominators()
            .iter()
            .chain(std::iter::once(family.domain().determinant_nonzero()))
            .try_fold(0_usize, |total, condition| {
                total.checked_add(condition.origins().len()).ok_or(
                    GenericTensorProjectorError::ResourceCountOverflow {
                        resource: "retained tensor projector family-domain origins",
                    },
                )
            })?;
        check_usize_limit(
            "retained tensor projector family-domain origins",
            family_domain_origins,
            self.limits.max_family_domain_origins,
        )?;
        let mut polynomial_terms = 0usize;
        let mut exponent_entries = 0usize;
        for condition in family
            .domain()
            .input_denominators()
            .iter()
            .chain(std::iter::once(family.domain().determinant_nonzero()))
        {
            polynomial_terms = polynomial_terms
                .checked_add(condition.polynomial().coefficients.len())
                .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "retained tensor projector family-domain polynomial terms",
                })?;
            exponent_entries = exponent_entries
                .checked_add(condition.polynomial().exponents.len())
                .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "retained tensor projector family-domain exponent entries",
                })?;
        }
        check_usize_limit(
            "retained tensor projector family-domain polynomial terms",
            polynomial_terms,
            self.limits.max_family_domain_polynomial_terms,
        )?;
        check_usize_limit(
            "retained tensor projector family-domain exponent entries",
            exponent_entries,
            self.limits.max_family_domain_exponent_entries,
        )?;
        Ok(family_domain_origins)
    }

    fn validate_source(
        &self,
        family: &IntegralFamily,
        source: &TensorMonomial,
    ) -> Result<(), GenericTensorProjectorError> {
        check_usize_limit(
            "tensor projector input vectors",
            source.vectors().len(),
            self.limits.max_input_vectors,
        )?;
        check_usize_limit(
            "tensor projector input metrics",
            source.metrics().len(),
            self.limits.max_input_metrics,
        )?;
        check_usize_limit(
            "tensor projector input scalar-product factors",
            source.scalar_products().factors().len(),
            self.limits.max_input_scalar_product_factors,
        )?;
        let endpoints = source
            .vectors()
            .len()
            .checked_add(source.metrics().len().checked_mul(2).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector index endpoints",
                },
            )?)
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector index endpoints",
            })?;
        check_usize_limit(
            "tensor projector index endpoints",
            endpoints,
            self.limits.max_index_endpoints,
        )?;

        for (position, vector) in source.vectors().iter().copied().enumerate() {
            validate_loop_vector(
                vector.vector(),
                family.loop_count(),
                TensorLoopReference::IndexedVector {
                    position,
                    index: vector.index(),
                },
            )?;
        }
        let mut degree = 0u64;
        for (&scalar_product, &exponent) in source.scalar_products().factors() {
            validate_loop_vector(
                scalar_product.left(),
                family.loop_count(),
                TensorLoopReference::ScalarProductLeft {
                    left: scalar_product.left(),
                    right: scalar_product.right(),
                },
            )?;
            validate_loop_vector(
                scalar_product.right(),
                family.loop_count(),
                TensorLoopReference::ScalarProductRight {
                    left: scalar_product.left(),
                    right: scalar_product.right(),
                },
            )?;
            degree = degree.checked_add(u64::from(exponent)).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector scalar-product degree",
                },
            )?;
        }
        if degree > self.limits.max_scalar_product_degree {
            return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                requested: degree,
                limit: self.limits.max_scalar_product_degree,
            });
        }
        Ok(())
    }

    fn contract_metrics(
        &self,
        family: &IntegralFamily,
        source: &TensorMonomial,
        budget: &mut ProjectionBudget,
    ) -> Result<VacuumMetricContractionWitness, GenericTensorProjectorError> {
        #[derive(Default)]
        struct IndexData {
            occurrences: usize,
            vectors: Vec<LoopVector>,
        }

        let mut index_data = BTreeMap::<LorentzIndex, IndexData>::new();
        for vector in source.vectors() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            let data = index_data.entry(vector.index()).or_default();
            data.occurrences = data.occurrences.checked_add(1).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector Lorentz-index occurrences",
                },
            )?;
            data.vectors.push(vector.vector());
        }
        for metric in source.metrics() {
            for index in [metric.left(), metric.right()] {
                charge_structural(budget, 1, self.limits.max_structural_operations)?;
                let data = index_data.entry(index).or_default();
                data.occurrences = data.occurrences.checked_add(1).ok_or(
                    GenericTensorProjectorError::ResourceCountOverflow {
                        resource: "tensor projector Lorentz-index occurrences",
                    },
                )?;
            }
        }
        for (&index, data) in &index_data {
            if data.occurrences > 2 {
                return Err(GenericTensorProjectorError::InvalidIndexMultiplicity {
                    index,
                    occurrences: data.occurrences,
                });
            }
        }

        let indices = index_data.keys().copied().collect::<Vec<_>>();
        let positions = indices
            .iter()
            .copied()
            .enumerate()
            .map(|(position, index)| (index, position))
            .collect::<BTreeMap<_, _>>();
        let mut components = ProjectionDisjointSet::new(indices.len());
        for metric in source.metrics() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            components.union(positions[&metric.left()], positions[&metric.right()]);
        }

        #[derive(Default)]
        struct Component {
            vectors: Vec<LoopVector>,
            free_indices: Vec<LorentzIndex>,
        }
        let mut grouped = BTreeMap::<usize, Component>::new();
        for (position, &index) in indices.iter().enumerate() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            let data = &index_data[&index];
            let component = grouped.entry(components.find(position)).or_default();
            component.vectors.extend(data.vectors.iter().copied());
            if data.occurrences == 1 {
                component.free_indices.push(index);
            }
        }

        let mut scalar_products = crate::GenericScalarProductMonomial::one();
        for (&scalar_product, &exponent) in source.scalar_products().factors() {
            scalar_products.try_multiply_power(
                ScalarProductCoordinate::LoopLoop {
                    left: usize::from(scalar_product.left().id()),
                    right: usize::from(scalar_product.right().id()),
                },
                exponent,
            )?;
        }

        let mut closed_metric_loops = 0u64;
        let mut vectors = Vec::new();
        let mut metrics = Vec::new();
        for mut component in grouped.into_values() {
            charge_structural(budget, 1, self.limits.max_structural_operations)?;
            component.vectors.sort_unstable();
            component.free_indices.sort_unstable();
            match (
                component.vectors.as_slice(),
                component.free_indices.as_slice(),
            ) {
                ([], []) => {
                    closed_metric_loops = closed_metric_loops.checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "closed metric loops",
                        },
                    )?;
                }
                ([], &[left, right]) => metrics.push(Metric::new(left, right)),
                (&[vector], &[index]) => vectors.push(IndexedVector::new(vector, index)),
                (&[left, right], []) => scalar_products.try_multiply_power(
                    ScalarProductCoordinate::LoopLoop {
                        left: usize::from(left.id()),
                        right: usize::from(right.id()),
                    },
                    1,
                )?,
                (vectors, free_indices) => {
                    return Err(GenericTensorProjectorError::InvalidMetricComponent {
                        vectors: vectors.len(),
                        free_indices: free_indices.len(),
                    });
                }
            }
        }
        vectors.sort_unstable();
        let degree = scalar_products.checked_degree()?;
        if degree > self.limits.max_scalar_product_degree {
            return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                requested: degree,
                limit: self.limits.max_scalar_product_degree,
            });
        }
        let coefficient = checked_pow(
            family.coefficient_context(),
            family.dimension(),
            closed_metric_loops,
            self.limits,
            budget,
        )?;
        Ok(VacuumMetricContractionWitness {
            closed_metric_loops,
            coefficient,
            vectors,
            metrics: MetricPairing::new(metrics),
            scalar_products,
        })
    }

    fn project_even_rank(
        &self,
        family: &IntegralFamily,
        contraction: &VacuumMetricContractionWitness,
        guards: &mut ProjectionGuardBuilder,
        budget: &mut ProjectionBudget,
    ) -> Result<
        (
            Vec<SlotPairing>,
            Vec<Vec<Coefficient>>,
            GenericTensorNumerator,
            usize,
        ),
        GenericTensorProjectorError,
    > {
        let rank = contraction.vectors.len();
        let pairing_count = perfect_matching_count(rank).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector perfect matchings",
            },
        )?;
        check_usize_limit(
            "tensor projector perfect matchings",
            pairing_count,
            self.limits.max_pairings,
        )?;
        charge_structural(
            budget,
            u64::try_from(pairing_count.checked_mul(rank).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector pairing enumeration work",
                },
            )?)
            .map_err(|_| GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector pairing enumeration work",
            })?,
            self.limits.max_structural_operations,
        )?;
        let pairings = perfect_matchings(rank, self.limits.max_pairings)?;

        let gram_entries = pairing_count.checked_mul(pairing_count).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector Gram entries",
            },
        )?;
        check_usize_limit(
            "tensor projector Gram entries",
            gram_entries,
            self.limits.max_gram_entries,
        )?;
        let augmented_entries = gram_entries.checked_mul(2).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector augmented entries",
            },
        )?;
        check_usize_limit(
            "tensor projector augmented entries",
            augmented_entries,
            self.limits.max_augmented_entries,
        )?;
        check_usize_limit(
            "tensor projector projection candidates",
            gram_entries,
            self.limits.max_projection_candidates,
        )?;

        let context = family.coefficient_context();
        let mut gram = Vec::with_capacity(pairing_count);
        for (row, left) in pairings.iter().enumerate() {
            let mut values = Vec::with_capacity(pairing_count);
            for (column, right) in pairings.iter().enumerate() {
                charge_structural(budget, 1, self.limits.max_structural_operations)?;
                let cycles = left.contraction_cycles(right)?;
                let coefficient = checked_pow(
                    context,
                    family.dimension(),
                    u64::try_from(cycles).map_err(|_| {
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "tensor projector contraction cycles",
                        }
                    })?,
                    self.limits,
                    budget,
                )?;
                guards.insert(
                    coefficient.denominator.clone(),
                    TensorProjectionGuardOrigin::GramEntryDenominator { row, column },
                    self.limits,
                    budget,
                )?;
                values.push(coefficient);
            }
            gram.push(values);
        }
        let inverse_gram = invert_checked_matrix(context, gram, rank, self.limits, guards, budget)?;
        for (row, values) in inverse_gram.iter().enumerate() {
            for (column, coefficient) in values.iter().enumerate() {
                guards.insert(
                    coefficient.denominator.clone(),
                    TensorProjectionGuardOrigin::InverseGramDenominator { row, column },
                    self.limits,
                    budget,
                )?;
                charge_coefficient_bytes(
                    coefficient,
                    budget,
                    self.limits.max_retained_coefficient_bytes,
                )?;
            }
        }

        let mut collected =
            BTreeMap::<(MetricPairing, crate::GenericScalarProductMonomial), Coefficient>::new();
        for (output_position, output_pairing) in pairings.iter().enumerate() {
            let mut metric_factors = contraction.metrics.metrics().to_vec();
            metric_factors.extend(output_pairing.pairs().iter().map(|&(left, right)| {
                Metric::new(
                    contraction.vectors[left].index(),
                    contraction.vectors[right].index(),
                )
            }));
            let metrics = MetricPairing::new(metric_factors);
            for (source_position, source_pairing) in pairings.iter().enumerate() {
                charge_structural(budget, 1, self.limits.max_structural_operations)?;
                let mut scalar_products = contraction.scalar_products.clone();
                for &(left, right) in source_pairing.pairs() {
                    scalar_products.try_multiply_power(
                        ScalarProductCoordinate::LoopLoop {
                            left: usize::from(contraction.vectors[left].vector().id()),
                            right: usize::from(contraction.vectors[right].vector().id()),
                        },
                        1,
                    )?;
                }
                let degree = scalar_products.checked_degree()?;
                if degree > self.limits.max_scalar_product_degree {
                    return Err(GenericTensorProjectorError::ScalarProductDegreeLimit {
                        requested: degree,
                        limit: self.limits.max_scalar_product_degree,
                    });
                }
                let coefficient = checked_mul(
                    context,
                    &contraction.coefficient,
                    &inverse_gram[output_position][source_position],
                    self.limits,
                    budget,
                )?;
                if coefficient.is_zero() {
                    continue;
                }
                let key = (metrics.clone(), scalar_products);
                if let Some(current) = collected.get(&key) {
                    let sum = checked_add(context, current, &coefficient, self.limits, budget)?;
                    if sum.is_zero() {
                        collected.remove(&key);
                    } else {
                        collected.insert(key, sum);
                    }
                } else {
                    let attempted = collected.len().checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "tensor projector output terms",
                        },
                    )?;
                    check_usize_limit(
                        "tensor projector output terms",
                        attempted,
                        self.limits.max_output_terms,
                    )?;
                    let structure_entries = key
                        .0
                        .metrics()
                        .len()
                        .checked_add(key.1.factors().len())
                        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "tensor projector output structure entries",
                        })?;
                    budget.consumed_output_structure_entries = budget
                        .consumed_output_structure_entries
                        .checked_add(structure_entries)
                        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "tensor projector output structure entries",
                        })?;
                    check_usize_limit(
                        "tensor projector output structure entries",
                        budget.consumed_output_structure_entries,
                        self.limits.max_output_structure_entries,
                    )?;
                    collected.insert(key, coefficient);
                }
            }
        }

        let mut terms = Vec::with_capacity(collected.len());
        for ((metrics, scalar_products), coefficient) in collected {
            let output_term = terms.len();
            guards.insert(
                coefficient.denominator.clone(),
                TensorProjectionGuardOrigin::ProjectedCoefficientDenominator { output_term },
                self.limits,
                budget,
            )?;
            charge_coefficient_bytes(
                &coefficient,
                budget,
                self.limits.max_retained_coefficient_bytes,
            )?;
            terms.push(GenericTensorTerm::new(
                coefficient,
                metrics,
                scalar_products,
            ));
        }
        let numerator =
            GenericTensorNumerator::try_new_with_limit(terms, self.limits.max_output_terms)?;
        Ok((pairings, inverse_gram, numerator, gram_entries))
    }
}

#[derive(Default)]
struct ProjectionBudget {
    arithmetic_operations: u64,
    symbolica_algebra_operations: u64,
    structural_operations: u64,
    consumed_output_structure_entries: usize,
    guard_polynomial_terms: usize,
    guard_exponent_entries: usize,
    guard_origins: usize,
    retained_coefficient_bytes: usize,
    matrix_input_retained_bytes: usize,
    matrix_output_retained_bytes: usize,
    matrix_peak_live_entries: usize,
}

#[derive(Default)]
struct ProjectionGuardBuilder {
    conditions: Vec<TensorProjectionNonZeroCondition>,
}

impl ProjectionGuardBuilder {
    fn insert(
        &mut self,
        polynomial: BasePolynomial,
        origin: TensorProjectionGuardOrigin,
        limits: GenericTensorProjectorLimits,
        budget: &mut ProjectionBudget,
    ) -> Result<(), GenericTensorProjectorError> {
        if polynomial.is_zero() {
            return Err(GenericTensorProjectorError::ZeroGuardPolynomial { origin });
        }
        if polynomial.is_constant() {
            return Ok(());
        }
        for condition in &mut self.conditions {
            charge_structural(budget, 1, limits.max_structural_operations)?;
            if condition.polynomial == polynomial {
                if !condition.origins.contains(&origin) {
                    let attempted = condition.origins.len().checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "tensor projector guard origins",
                        },
                    )?;
                    check_usize_limit(
                        "tensor projector guard origins",
                        attempted,
                        limits.max_guard_origins_per_condition,
                    )?;
                    let aggregate = budget.guard_origins.checked_add(1).ok_or(
                        GenericTensorProjectorError::ResourceCountOverflow {
                            resource: "retained tensor projector guard origins",
                        },
                    )?;
                    check_usize_limit(
                        "retained tensor projector guard origins",
                        aggregate,
                        limits.max_guard_origins,
                    )?;
                    condition.origins.insert(origin);
                    budget.guard_origins = aggregate;
                }
                return Ok(());
            }
        }
        let attempted = self.conditions.len().checked_add(1).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector nonzero conditions",
            },
        )?;
        check_usize_limit(
            "tensor projector nonzero conditions",
            attempted,
            limits.max_nonzero_conditions,
        )?;
        check_usize_limit(
            "tensor projector guard origins",
            1,
            limits.max_guard_origins_per_condition,
        )?;
        let aggregate = budget.guard_origins.checked_add(1).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "retained tensor projector guard origins",
            },
        )?;
        check_usize_limit(
            "retained tensor projector guard origins",
            aggregate,
            limits.max_guard_origins,
        )?;
        budget.guard_polynomial_terms = budget
            .guard_polynomial_terms
            .checked_add(polynomial.coefficients.len())
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector guard polynomial terms",
            })?;
        budget.guard_exponent_entries = budget
            .guard_exponent_entries
            .checked_add(polynomial.exponents.len())
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector guard exponent entries",
            })?;
        check_usize_limit(
            "tensor projector guard polynomial terms",
            budget.guard_polynomial_terms,
            limits.max_guard_polynomial_terms,
        )?;
        check_usize_limit(
            "tensor projector guard exponent entries",
            budget.guard_exponent_entries,
            limits.max_guard_exponent_entries,
        )?;
        self.conditions.push(TensorProjectionNonZeroCondition {
            polynomial,
            origins: BTreeSet::from([origin]),
        });
        budget.guard_origins = aggregate;
        Ok(())
    }
}

fn build_projector_data(
    family: &IntegralFamily,
    rank: usize,
    limits: GenericTensorProjectorLimits,
    guards: &mut ProjectionGuardBuilder,
    budget: &mut ProjectionBudget,
) -> Result<(Vec<SlotPairing>, Vec<Vec<Coefficient>>, usize), GenericTensorProjectorError> {
    let pairing_count =
        perfect_matching_count(rank).ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector perfect matchings",
        })?;
    check_usize_limit(
        "tensor projector perfect matchings",
        pairing_count,
        limits.max_pairings,
    )?;
    charge_structural(
        budget,
        u64::try_from(pairing_count.checked_mul(rank).ok_or(
            GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector pairing enumeration work",
            },
        )?)
        .map_err(|_| GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector pairing enumeration work",
        })?,
        limits.max_structural_operations,
    )?;
    let pairings = perfect_matchings(rank, limits.max_pairings)?;
    let gram_entries = pairing_count.checked_mul(pairing_count).ok_or(
        GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Gram entries",
        },
    )?;
    check_usize_limit(
        "tensor projector Gram entries",
        gram_entries,
        limits.max_gram_entries,
    )?;
    let augmented_entries =
        gram_entries
            .checked_mul(2)
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
                resource: "tensor projector augmented entries",
            })?;
    check_usize_limit(
        "tensor projector augmented entries",
        augmented_entries,
        limits.max_augmented_entries,
    )?;
    check_usize_limit(
        "tensor projector projection candidates",
        gram_entries,
        limits.max_projection_candidates,
    )?;

    let context = family.coefficient_context();
    let mut gram = Vec::with_capacity(pairing_count);
    for (row, left) in pairings.iter().enumerate() {
        let mut values = Vec::with_capacity(pairing_count);
        for (column, right) in pairings.iter().enumerate() {
            charge_structural(budget, 1, limits.max_structural_operations)?;
            let cycles = left.contraction_cycles(right)?;
            let coefficient = checked_pow(
                context,
                family.dimension(),
                u64::try_from(cycles).map_err(|_| {
                    GenericTensorProjectorError::ResourceCountOverflow {
                        resource: "tensor projector contraction cycles",
                    }
                })?,
                limits,
                budget,
            )?;
            guards.insert(
                coefficient.denominator.clone(),
                TensorProjectionGuardOrigin::GramEntryDenominator { row, column },
                limits,
                budget,
            )?;
            values.push(coefficient);
        }
        gram.push(values);
    }
    let inverse_gram = invert_checked_matrix(context, gram, rank, limits, guards, budget)?;
    for (row, values) in inverse_gram.iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            guards.insert(
                coefficient.denominator.clone(),
                TensorProjectionGuardOrigin::InverseGramDenominator { row, column },
                limits,
                budget,
            )?;
            charge_coefficient_bytes(coefficient, budget, limits.max_retained_coefficient_bytes)?;
        }
    }
    Ok((pairings, inverse_gram, gram_entries))
}

fn invert_checked_matrix(
    context: &CoefficientContext,
    matrix: Vec<Vec<Coefficient>>,
    rank: usize,
    limits: GenericTensorProjectorLimits,
    guards: &mut ProjectionGuardBuilder,
    budget: &mut ProjectionBudget,
) -> Result<Vec<Vec<Coefficient>>, GenericTensorProjectorError> {
    let session_limits = remaining_symbolica_limits(limits, budget)?;
    let verified = invert_and_verify_coefficient_matrix(context, &matrix, session_limits)
        .map_err(|error| map_symbolica_algebra_error(error, Some(rank), limits, budget))?;
    let (inverse, determinant, stats) = verified.into_parts();
    absorb_symbolica_stats(stats, limits, budget)?;
    guards.insert(
        determinant.numerator.clone(),
        TensorProjectionGuardOrigin::ProjectorGramDeterminantNumerator { rank },
        limits,
        budget,
    )?;
    guards.insert(
        determinant.denominator.clone(),
        TensorProjectionGuardOrigin::ProjectorGramDeterminantDenominator { rank },
        limits,
        budget,
    )?;
    Ok(inverse)
}

fn checked_add(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: GenericTensorProjectorLimits,
    budget: &mut ProjectionBudget,
) -> Result<Coefficient, GenericTensorProjectorError> {
    charge_arithmetic(budget, limits.max_arithmetic_operations)?;
    Ok(context.try_add(left, right, limits.exact_algebra)?)
}

fn checked_mul(
    context: &CoefficientContext,
    left: &Coefficient,
    right: &Coefficient,
    limits: GenericTensorProjectorLimits,
    budget: &mut ProjectionBudget,
) -> Result<Coefficient, GenericTensorProjectorError> {
    charge_arithmetic(budget, limits.max_arithmetic_operations)?;
    Ok(context.try_mul(left, right, limits.exact_algebra)?)
}

fn checked_pow(
    context: &CoefficientContext,
    base: &Coefficient,
    exponent: u64,
    limits: GenericTensorProjectorLimits,
    budget: &mut ProjectionBudget,
) -> Result<Coefficient, GenericTensorProjectorError> {
    let session_limits = remaining_symbolica_limits(limits, budget)?;
    let (result, stats) = power_of_coefficient(context, base, exponent, session_limits)
        .map_err(|error| map_symbolica_algebra_error(error, None, limits, budget))?;
    absorb_symbolica_stats(stats, limits, budget)?;
    Ok(result)
}

fn remaining_symbolica_limits(
    limits: GenericTensorProjectorLimits,
    budget: &ProjectionBudget,
) -> Result<SymbolicaCoefficientMatrixLimits, GenericTensorProjectorError> {
    let remaining_arithmetic = limits
        .max_arithmetic_operations
        .checked_sub(budget.arithmetic_operations)
        .ok_or(GenericTensorProjectorError::ArithmeticOperationLimit {
            attempted: budget.arithmetic_operations,
            limit: limits.max_arithmetic_operations,
        })?;
    let remaining_input = limits
        .max_matrix_input_retained_bytes
        .checked_sub(budget.matrix_input_retained_bytes)
        .ok_or(GenericTensorProjectorError::ResourceLimit {
            resource: "tensor projector Symbolica input retained bytes",
            requested: budget.matrix_input_retained_bytes,
            limit: limits.max_matrix_input_retained_bytes,
        })?;
    let remaining_output = limits
        .max_matrix_output_retained_bytes
        .checked_sub(budget.matrix_output_retained_bytes)
        .ok_or(GenericTensorProjectorError::ResourceLimit {
            resource: "tensor projector Symbolica output retained bytes",
            requested: budget.matrix_output_retained_bytes,
            limit: limits.max_matrix_output_retained_bytes,
        })?;
    Ok(SymbolicaCoefficientMatrixLimits {
        exact_algebra: limits.exact_algebra,
        max_single_matrix_entries: limits.max_augmented_entries,
        max_live_matrix_entries: limits.max_matrix_live_entries,
        max_exact_operations: usize::try_from(remaining_arithmetic).unwrap_or(usize::MAX),
        max_input_retained_bytes: remaining_input,
        max_output_retained_bytes: remaining_output,
    })
}

fn absorb_symbolica_stats(
    stats: SymbolicaCoefficientMatrixStats,
    limits: GenericTensorProjectorLimits,
    budget: &mut ProjectionBudget,
) -> Result<(), GenericTensorProjectorError> {
    let exact_operations = u64::try_from(stats.exact_operations()).map_err(|_| {
        GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Symbolica exact operations",
        }
    })?;
    let arithmetic_operations = budget
        .arithmetic_operations
        .checked_add(exact_operations)
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector arithmetic operations",
        })?;
    if arithmetic_operations > limits.max_arithmetic_operations {
        return Err(GenericTensorProjectorError::ArithmeticOperationLimit {
            attempted: arithmetic_operations,
            limit: limits.max_arithmetic_operations,
        });
    }
    let symbolica_algebra_operations = budget
        .symbolica_algebra_operations
        .checked_add(exact_operations)
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Symbolica exact operations",
        })?;
    let matrix_input_retained_bytes = budget
        .matrix_input_retained_bytes
        .checked_add(stats.input_retained_bytes())
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Symbolica input retained bytes",
        })?;
    check_usize_limit(
        "tensor projector Symbolica input retained bytes",
        matrix_input_retained_bytes,
        limits.max_matrix_input_retained_bytes,
    )?;
    let matrix_output_retained_bytes = budget
        .matrix_output_retained_bytes
        .checked_add(stats.output_retained_bytes())
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Symbolica output retained bytes",
        })?;
    check_usize_limit(
        "tensor projector Symbolica output retained bytes",
        matrix_output_retained_bytes,
        limits.max_matrix_output_retained_bytes,
    )?;
    check_usize_limit(
        "tensor projector Symbolica matrix peak live entries",
        stats.admitted_peak_live_entries(),
        limits.max_matrix_live_entries,
    )?;

    budget.arithmetic_operations = arithmetic_operations;
    budget.symbolica_algebra_operations = symbolica_algebra_operations;
    budget.matrix_input_retained_bytes = matrix_input_retained_bytes;
    budget.matrix_output_retained_bytes = matrix_output_retained_bytes;
    budget.matrix_peak_live_entries = budget
        .matrix_peak_live_entries
        .max(stats.admitted_peak_live_entries());
    Ok(())
}

fn map_symbolica_algebra_error(
    error: SymbolicaCoefficientMatrixError,
    projector_rank: Option<usize>,
    limits: GenericTensorProjectorLimits,
    budget: &ProjectionBudget,
) -> GenericTensorProjectorError {
    match error {
        SymbolicaCoefficientMatrixError::Singular => match projector_rank {
            Some(rank) => GenericTensorProjectorError::SingularProjector { rank },
            None => GenericTensorProjectorError::InternalSymbolicaAlgebra {
                detail: "Symbolica reported singularity during scalar exponentiation".to_owned(),
            },
        },
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => map_symbolica_resource_limit(resource, requested, limit, limits, budget),
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(
            ExactAlgebraError::ResourceCountOverflow { resource },
        )
        | SymbolicaCoefficientMatrixError::InvalidCoefficient {
            error: ExactAlgebraError::ResourceCountOverflow { resource },
            ..
        } => GenericTensorProjectorError::ResourceCountOverflow { resource },
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => GenericTensorProjectorError::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::ExactAlgebra(error)
        | SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. } => {
            map_symbolica_exact_error(error, limits, budget)
        }
        internal => GenericTensorProjectorError::InternalSymbolicaAlgebra {
            detail: internal.to_string(),
        },
    }
}

fn map_symbolica_exact_error(
    error: ExactAlgebraError,
    limits: GenericTensorProjectorLimits,
    budget: &ProjectionBudget,
) -> GenericTensorProjectorError {
    match error {
        ExactAlgebraError::ResourceLimit {
            resource,
            requested,
            limit,
        } if is_symbolica_operation_resource(resource) => {
            map_symbolica_resource_limit(resource, requested, limit, limits, budget)
        }
        other => GenericTensorProjectorError::ExactAlgebra(other),
    }
}

fn map_symbolica_resource_limit(
    resource: &'static str,
    requested: usize,
    local_limit: usize,
    limits: GenericTensorProjectorLimits,
    budget: &ProjectionBudget,
) -> GenericTensorProjectorError {
    if is_symbolica_operation_resource(resource) {
        let Ok(requested) = u64::try_from(requested) else {
            return GenericTensorProjectorError::ResourceCountOverflow { resource };
        };
        return match budget.arithmetic_operations.checked_add(requested) {
            Some(attempted) => GenericTensorProjectorError::ArithmeticOperationLimit {
                attempted,
                limit: limits.max_arithmetic_operations,
            },
            None => GenericTensorProjectorError::ResourceCountOverflow { resource },
        };
    }
    let (current, global_limit) = if is_symbolica_input_byte_resource(resource) {
        (
            budget.matrix_input_retained_bytes,
            limits.max_matrix_input_retained_bytes,
        )
    } else if is_symbolica_output_byte_resource(resource) {
        (
            budget.matrix_output_retained_bytes,
            limits.max_matrix_output_retained_bytes,
        )
    } else {
        return GenericTensorProjectorError::ResourceLimit {
            resource,
            requested,
            limit: local_limit,
        };
    };
    match current.checked_add(requested) {
        Some(requested) => GenericTensorProjectorError::ResourceLimit {
            resource,
            requested,
            limit: global_limit,
        },
        None => GenericTensorProjectorError::ResourceCountOverflow { resource },
    }
}

fn is_symbolica_operation_resource(resource: &str) -> bool {
    resource.starts_with("Symbolica coefficient") && resource.ends_with("exact operations")
}

fn is_symbolica_input_byte_resource(resource: &str) -> bool {
    resource.contains("input retained bytes")
}

fn is_symbolica_output_byte_resource(resource: &str) -> bool {
    resource.contains("output retained bytes")
}

fn charge_arithmetic(
    budget: &mut ProjectionBudget,
    limit: u64,
) -> Result<(), GenericTensorProjectorError> {
    charge_arithmetic_amount(budget, 1, limit)
}

fn charge_arithmetic_amount(
    budget: &mut ProjectionBudget,
    amount: u64,
    limit: u64,
) -> Result<(), GenericTensorProjectorError> {
    let attempted = budget.arithmetic_operations.checked_add(amount).ok_or(
        GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector arithmetic operations",
        },
    )?;
    if attempted > limit {
        return Err(GenericTensorProjectorError::ArithmeticOperationLimit { attempted, limit });
    }
    budget.arithmetic_operations = attempted;
    Ok(())
}

fn charge_structural(
    budget: &mut ProjectionBudget,
    amount: u64,
    limit: u64,
) -> Result<(), GenericTensorProjectorError> {
    let attempted = budget.structural_operations.checked_add(amount).ok_or(
        GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector structural operations",
        },
    )?;
    if attempted > limit {
        return Err(GenericTensorProjectorError::StructuralOperationLimit { attempted, limit });
    }
    budget.structural_operations = attempted;
    Ok(())
}

fn validate_loop_vector(
    vector: LoopVector,
    loops: usize,
    location: TensorLoopReference,
) -> Result<(), GenericTensorProjectorError> {
    if usize::from(vector.id()) >= loops {
        Err(GenericTensorProjectorError::LoopVectorOutOfRange {
            vector,
            loops,
            location,
        })
    } else {
        Ok(())
    }
}

fn manifest_bytes(
    family: &IntegralFamily,
    fingerprint: &str,
) -> Result<usize, GenericTensorProjectorError> {
    family
        .loop_momenta()
        .iter()
        .try_fold(fingerprint.len(), |bytes, label| {
            bytes.checked_add(label.len()).ok_or(
                GenericTensorProjectorError::ResourceCountOverflow {
                    resource: "tensor projector family manifest bytes",
                },
            )
        })
}

struct BoundedLengthWriter {
    length: usize,
    limit: usize,
}

impl Write for BoundedLengthWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if next > self.limit {
            return Err(fmt::Error);
        }
        self.length = next;
        Ok(())
    }
}

fn charge_coefficient_bytes(
    coefficient: &Coefficient,
    budget: &mut ProjectionBudget,
    limit: usize,
) -> Result<(), GenericTensorProjectorError> {
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: limit.saturating_sub(budget.retained_coefficient_bytes),
    };
    write!(&mut writer, "{coefficient}").map_err(|_| {
        GenericTensorProjectorError::ResourceLimit {
            resource: "retained tensor projector coefficient bytes",
            requested: limit.saturating_add(1),
            limit,
        }
    })?;
    budget.retained_coefficient_bytes = budget
        .retained_coefficient_bytes
        .checked_add(writer.length)
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "retained tensor projector coefficient bytes",
        })?;
    Ok(())
}

fn check_usize_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GenericTensorProjectorError> {
    if requested > limit {
        Err(GenericTensorProjectorError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_occurrence(current: usize) -> Result<usize, GenericTensorProjectorError> {
    current
        .checked_add(1)
        .ok_or(GenericTensorProjectorError::ResourceCountOverflow {
            resource: "tensor projector Lorentz-index occurrences",
        })
}

fn bounded_collect<T>(
    values: impl IntoIterator<Item = T>,
    resource: &'static str,
    limit: usize,
) -> Result<Vec<T>, GenericTensorProjectorError> {
    let mut retained = Vec::new();
    for value in values {
        let attempted = retained
            .len()
            .checked_add(1)
            .ok_or(GenericTensorProjectorError::ResourceCountOverflow { resource })?;
        check_usize_limit(resource, attempted, limit)?;
        retained.push(value);
    }
    Ok(retained)
}

#[derive(Debug)]
struct ProjectionDisjointSet {
    parent: Vec<usize>,
}

impl ProjectionDisjointSet {
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

/// Typed failures from authenticated generic tensor projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenericTensorProjectorError {
    ExternalMomentaUnsupported {
        externals: usize,
    },
    LoopVectorOutOfRange {
        vector: LoopVector,
        loops: usize,
        location: TensorLoopReference,
    },
    InvalidIndexMultiplicity {
        index: LorentzIndex,
        occurrences: usize,
    },
    AmbiguousSameIndexVectorContraction {
        index: LorentzIndex,
        loop_vectors: usize,
        spectator_vectors: usize,
    },
    InvalidMetricComponent {
        vectors: usize,
        free_indices: usize,
    },
    InvalidCovariantMetricComponent {
        loop_vectors: usize,
        spectator_vectors: usize,
        free_indices: usize,
    },
    ScalarProductDegreeLimit {
        requested: u64,
        limit: u64,
    },
    SpectatorScalarProductDegreeLimit {
        requested: u64,
        limit: u64,
    },
    SpectatorScalarProductExponentOverflow {
        factor: SpectatorScalarProduct,
    },
    SpectatorCovariantCannotUseMetricBridge {
        term: usize,
    },
    SingularProjector {
        rank: usize,
    },
    ZeroGuardPolynomial {
        origin: TensorProjectionGuardOrigin,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ArithmeticOperationLimit {
        attempted: u64,
        limit: u64,
    },
    StructuralOperationLimit {
        attempted: u64,
        limit: u64,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    WrongFamilyFingerprint {
        expected: Arc<str>,
        actual: Arc<str>,
    },
    ExactAlgebra(ExactAlgebraError),
    Tensor(TensorError),
    GenericTensor(GenericTensorFamilyError),
    InternalSymbolicaAlgebra {
        detail: String,
    },
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for GenericTensorProjectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExternalMomentaUnsupported { externals } => write!(
                formatter,
                "vacuum tensor projection cannot authenticate a family with {externals} external momenta"
            ),
            Self::LoopVectorOutOfRange {
                vector,
                loops,
                location,
            } => write!(
                formatter,
                "loop-vector id {} at {location:?} is outside the family's zero-based ordered basis of {loops} loops",
                vector.id()
            ),
            Self::InvalidIndexMultiplicity { index, occurrences } => write!(
                formatter,
                "Lorentz index {} occurs {occurrences} times; expected once or twice",
                index.id()
            ),
            Self::AmbiguousSameIndexVectorContraction {
                index,
                loop_vectors,
                spectator_vectors,
            } => write!(
                formatter,
                "Lorentz index {} has {loop_vectors} loop-vector and {spectator_vectors} spectator-vector endpoints; Vakint-compatible precontraction has no unique vector pair",
                index.id()
            ),
            Self::InvalidMetricComponent {
                vectors,
                free_indices,
            } => write!(
                formatter,
                "invalid metric-contraction component with {vectors} vector endpoints and {free_indices} free indices"
            ),
            Self::InvalidCovariantMetricComponent {
                loop_vectors,
                spectator_vectors,
                free_indices,
            } => write!(
                formatter,
                "invalid covariant metric component with {loop_vectors} loop vectors, {spectator_vectors} spectator vectors, and {free_indices} free indices"
            ),
            Self::ScalarProductDegreeLimit { requested, limit } => write!(
                formatter,
                "projected scalar-product degree {requested} exceeds limit {limit}"
            ),
            Self::SpectatorScalarProductDegreeLimit { requested, limit } => write!(
                formatter,
                "projected spectator scalar-product degree {requested} exceeds limit {limit}"
            ),
            Self::SpectatorScalarProductExponentOverflow { factor } => write!(
                formatter,
                "the exponent of spectator scalar product ({},{}) overflows u32",
                factor.left().id(),
                factor.right().id()
            ),
            Self::SpectatorCovariantCannotUseMetricBridge { term } => write!(
                formatter,
                "covariant tensor term {term} contains spectator vectors or scalar products and cannot use the metric-only family bridge"
            ),
            Self::SingularProjector { rank } => write!(
                formatter,
                "rank-{rank} metric Gram matrix is identically singular"
            ),
            Self::ZeroGuardPolynomial { origin } => write!(
                formatter,
                "tensor projection produced an invalid identically-zero guard at {origin:?}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ArithmeticOperationLimit { attempted, limit } => write!(
                formatter,
                "tensor projection needs at least {attempted} exact operations, exceeding limit {limit}"
            ),
            Self::StructuralOperationLimit { attempted, limit } => write!(
                formatter,
                "tensor projection needs at least {attempted} structural operations, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "failed to reserve {requested} {resource} while building the tensor projector"
            ),
            Self::WrongFamilyFingerprint { expected, actual } => write!(
                formatter,
                "tensor projection belongs to family fingerprint {expected:?}, not {actual:?}"
            ),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Tensor(error) => error.fmt(formatter),
            Self::GenericTensor(error) => error.fmt(formatter),
            Self::InternalSymbolicaAlgebra { detail } => write!(
                formatter,
                "authenticated Symbolica tensor-projector algebra failed: {detail}"
            ),
            Self::InternalVerificationFailure { detail } => {
                write!(
                    formatter,
                    "authenticated tensor projection replay failed: {detail}"
                )
            }
        }
    }
}

impl Error for GenericTensorProjectorError {}

impl From<ExactAlgebraError> for GenericTensorProjectorError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

impl From<TensorError> for GenericTensorProjectorError {
    fn from(value: TensorError) -> Self {
        Self::Tensor(value)
    }
}

impl From<GenericTensorFamilyError> for GenericTensorProjectorError {
    fn from(value: GenericTensorFamilyError) -> Self {
        Self::GenericTensor(value)
    }
}
