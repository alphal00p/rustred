//! Generic bridge from scalar-product tensor numerators to the parametric
//! integral lattice.
//!
//! Tensor projection and scalar IBP reduction are deliberately separate
//! operations.  This module implements the exact middle step for an
//! [`IntegralFamily`](crate::IntegralFamily): every loop--loop or
//! loop--external scalar product is expanded in the family's authenticated
//! affine denominator basis, and every denominator monomial is converted to a
//! checked [`ConcreteIntegralKey`](crate::ConcreteIntegralKey).  No recurrence
//! or topology-specific rule is embedded here.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::generic_family::BasePolynomial as FamilyBasePolynomial;
use crate::{
    ConcreteIntegralKey, FamilyDomain, GenericFamilyError, IntegralFamily, MetricPairing,
    ParametricRelationError, ScalarProductCoordinate, TensorReduction, algebra::Coefficient,
    algebra::ExactAlgebraError, algebra::ExactAlgebraLimits,
};

/// Stable semantic version of the generic tensor-to-family bridge.
pub const GENERIC_TENSOR_FAMILY_LOWERING_V1_SCHEMA: &str =
    "rustred-generic-tensor-family-lowering-v1";

/// Resource policy for one tensor-numerator lowering call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericTensorFamilyLimits {
    /// Limits applied to every Symbolica rational-polynomial operation.
    pub exact_algebra: ExactAlgebraLimits,
    /// Maximum number of input tensor terms.
    pub max_input_terms: usize,
    /// Maximum total scalar-product degree in one input monomial.
    pub max_scalar_product_degree: u64,
    /// Maximum number of factor entries consumed by a monomial constructor.
    pub max_scalar_product_factor_entries: usize,
    /// Maximum number of distinct scalar-product coordinates in one term.
    pub max_distinct_scalar_products_per_input: usize,
    /// Maximum number of metric factors retained in one input term.
    pub max_metrics_per_input: usize,
    /// Aggregate metric and scalar-product map entries retained in the source
    /// manifest.
    pub max_source_structure_entries: usize,
    /// Maximum number of denominator monomials retained while expanding one
    /// input scalar-product monomial.
    pub max_expansion_terms_per_input: usize,
    /// Maximum number of retained `(metric structure, integral)` pairs.
    pub max_output_terms: usize,
    /// Maximum denominator-exponent entries in one live expansion polynomial.
    pub max_expansion_exponent_entries: usize,
    /// Maximum denominator-exponent entries in the retained output keys.
    pub max_output_exponent_entries: usize,
    /// Maximum number of affine multiplication/collection operations.
    pub max_expansion_operations: u64,
    /// Maximum number of distinct input origins retained for one output term.
    pub max_origins_per_output: usize,
    /// Maximum number of output origins retained across the complete call.
    pub max_retained_origins: usize,
    /// Maximum distinct input-coefficient denominator conditions.
    pub max_nonzero_conditions: usize,
    /// Maximum input origins merged into one nonzero condition.
    pub max_origins_per_nonzero_condition: usize,
    /// Maximum input origins retained across all nonzero conditions.
    pub max_nonzero_condition_origins: usize,
    /// Maximum number of complete covariant keys retained by a dispatcher
    /// built on this lowering policy.
    pub max_covariant_structures: usize,
    /// Aggregate metric, spectator-vector, and spectator-scalar-product
    /// entries retained in complete covariant keys.
    pub max_covariant_structure_entries: usize,
    /// Aggregate bounded Debug bytes of retained complete covariant keys.
    pub max_covariant_structure_bytes: usize,
    /// Maximum number of complete [`FamilyDomain`] copies retained by a
    /// covariant dispatcher. A scalar lowering retains one such copy for every
    /// complete covariant key.
    pub max_family_domain_copies: usize,
    /// Aggregate family-domain condition records across those retained copies.
    pub max_family_domain_conditions: usize,
    /// Aggregate typed origins carried by the retained family-domain
    /// conditions.
    pub max_family_domain_origins: usize,
    /// Aggregate base-polynomial terms retained by the family-domain
    /// conditions and basis determinant.
    pub max_family_domain_polynomial_terms: usize,
    /// Aggregate base-polynomial exponent entries retained by the
    /// family-domain conditions and basis determinant.
    pub max_family_domain_exponent_entries: usize,
    /// Aggregate bytes of the family fingerprints independently retained by
    /// the per-covariant scalar lowerings.
    pub max_family_manifest_bytes: usize,
    /// Maximum formatted bytes of retained source and output coefficients.
    pub max_retained_coefficient_bytes: usize,
}

impl Default for GenericTensorFamilyLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_input_terms: 100_000,
            max_scalar_product_degree: u64::from(u16::MAX),
            max_scalar_product_factor_entries: 4_096,
            max_distinct_scalar_products_per_input: 4_096,
            max_metrics_per_input: 4_096,
            max_source_structure_entries: 16_000_000,
            max_expansion_terms_per_input: 1_000_000,
            max_output_terms: 1_000_000,
            max_expansion_exponent_entries: 64_000_000,
            max_output_exponent_entries: 64_000_000,
            max_expansion_operations: 10_000_000,
            max_origins_per_output: 100_000,
            max_retained_origins: 100_000_000,
            max_nonzero_conditions: 100_000,
            max_origins_per_nonzero_condition: 100_000,
            max_nonzero_condition_origins: 100_000_000,
            max_covariant_structures: 1_000_000,
            max_covariant_structure_entries: 64_000_000,
            max_covariant_structure_bytes: 256 * 1024 * 1024,
            max_family_domain_copies: 1_000_000,
            max_family_domain_conditions: 10_000_000,
            max_family_domain_origins: 100_000_000,
            max_family_domain_polynomial_terms: 100_000_000,
            max_family_domain_exponent_entries: 1_000_000_000,
            max_family_manifest_bytes: 256 * 1024 * 1024,
            max_retained_coefficient_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Auditable work and retained-data counters for one generic lowering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenericTensorFamilyStats {
    pub input_terms: usize,
    pub source_structure_entries: usize,
    pub expansion_operations: u64,
    pub output_terms: usize,
    pub output_exponent_entries: usize,
    pub retained_origins: usize,
    pub nonzero_conditions: usize,
    pub nonzero_condition_origins: usize,
    pub retained_coefficient_bytes: usize,
}

/// A canonical monomial in the scalar-product coordinates owned by a generic
/// integral family.
///
/// Coordinates are validated against the concrete family at lowering time,
/// so the same numerator object can be reused with compatible family
/// definitions.  Repeated factors are collected with checked `u32` arithmetic.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GenericScalarProductMonomial {
    factors: BTreeMap<ScalarProductCoordinate, u32>,
}

impl GenericScalarProductMonomial {
    pub fn one() -> Self {
        Self::default()
    }

    pub fn try_from_factors(
        factors: impl IntoIterator<Item = (ScalarProductCoordinate, u32)>,
    ) -> Result<Self, GenericTensorFamilyError> {
        let defaults = GenericTensorFamilyLimits::default();
        Self::try_from_factors_with_limits(
            factors,
            defaults.max_scalar_product_factor_entries,
            defaults.max_scalar_product_degree,
        )
    }

    pub fn try_from_factors_with_limits(
        factors: impl IntoIterator<Item = (ScalarProductCoordinate, u32)>,
        max_factor_entries: usize,
        max_degree: u64,
    ) -> Result<Self, GenericTensorFamilyError> {
        let mut monomial = Self::one();
        let mut entries = 0_usize;
        let mut degree = 0_u64;
        for (coordinate, exponent) in factors {
            entries =
                entries
                    .checked_add(1)
                    .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                        resource: "tensor scalar-product factor entries",
                    })?;
            check_usize_limit(
                "tensor scalar-product factor entries",
                entries,
                max_factor_entries,
            )?;
            degree = degree.checked_add(u64::from(exponent)).ok_or(
                GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "tensor scalar-product degree",
                },
            )?;
            if degree > max_degree {
                return Err(GenericTensorFamilyError::ConstructorDegreeLimit {
                    requested: degree,
                    limit: max_degree,
                });
            }
            monomial.try_multiply_power(coordinate, exponent)?;
        }
        Ok(monomial)
    }

    pub fn factors(&self) -> &BTreeMap<ScalarProductCoordinate, u32> {
        &self.factors
    }

    pub fn exponent(&self, coordinate: ScalarProductCoordinate) -> u32 {
        let coordinate = canonical_scalar_product_coordinate(coordinate);
        self.factors.get(&coordinate).copied().unwrap_or(0)
    }

    pub fn checked_degree(&self) -> Result<u64, GenericTensorFamilyError> {
        self.factors.values().try_fold(0_u64, |degree, &exponent| {
            degree.checked_add(u64::from(exponent)).ok_or(
                GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "tensor scalar-product degree",
                },
            )
        })
    }

    pub fn is_one(&self) -> bool {
        self.factors.is_empty()
    }

    pub fn try_multiply_power(
        &mut self,
        coordinate: ScalarProductCoordinate,
        exponent: u32,
    ) -> Result<(), GenericTensorFamilyError> {
        if exponent == 0 {
            return Ok(());
        }
        let coordinate = canonical_scalar_product_coordinate(coordinate);
        let current = self.factors.entry(coordinate).or_default();
        *current = current
            .checked_add(exponent)
            .ok_or(GenericTensorFamilyError::ScalarProductExponentOverflow { coordinate })?;
        Ok(())
    }
}

fn canonical_scalar_product_coordinate(
    coordinate: ScalarProductCoordinate,
) -> ScalarProductCoordinate {
    match coordinate {
        ScalarProductCoordinate::LoopLoop { left, right } if left > right => {
            ScalarProductCoordinate::LoopLoop {
                left: right,
                right: left,
            }
        }
        coordinate => coordinate,
    }
}

/// One already-projected tensor term.  The free Lorentz structure is retained
/// independently of the scalar-product numerator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericTensorTerm {
    coefficient: Coefficient,
    metrics: MetricPairing,
    scalar_products: GenericScalarProductMonomial,
}

impl GenericTensorTerm {
    pub fn new(
        coefficient: Coefficient,
        metrics: MetricPairing,
        scalar_products: GenericScalarProductMonomial,
    ) -> Self {
        Self {
            coefficient,
            metrics,
            scalar_products,
        }
    }

    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn metrics(&self) -> &MetricPairing {
        &self.metrics
    }

    pub fn scalar_products(&self) -> &GenericScalarProductMonomial {
        &self.scalar_products
    }
}

/// A sparse sum of projected tensor terms ready for family lowering.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenericTensorNumerator {
    terms: Vec<GenericTensorTerm>,
}

impl GenericTensorNumerator {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn try_new(
        terms: impl IntoIterator<Item = GenericTensorTerm>,
    ) -> Result<Self, GenericTensorFamilyError> {
        Self::try_new_with_limit(terms, GenericTensorFamilyLimits::default().max_input_terms)
    }

    pub fn try_new_with_limit(
        terms: impl IntoIterator<Item = GenericTensorTerm>,
        max_input_terms: usize,
    ) -> Result<Self, GenericTensorFamilyError> {
        let mut retained = Vec::new();
        for term in terms {
            let attempted = retained.len().checked_add(1).ok_or(
                GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "tensor input terms",
                },
            )?;
            check_usize_limit("tensor input terms", attempted, max_input_terms)?;
            retained.push(term);
        }
        Ok(Self { terms: retained })
    }

    pub fn terms(&self) -> &[GenericTensorTerm] {
        &self.terms
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }
}

/// Auditable source of a retained lowered coefficient.
///
/// `input_term` is the stable ordinal in [`GenericTensorNumerator::terms`].
/// The complete scalar-product monomial is copied so a downstream reduction
/// trace can identify exactly which numerator term produced a scalar integral.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TensorLoweringOrigin {
    input_term: usize,
    scalar_products: GenericScalarProductMonomial,
}

impl TensorLoweringOrigin {
    pub fn input_term(&self) -> usize {
        self.input_term
    }

    pub fn scalar_products(&self) -> &GenericScalarProductMonomial {
        &self.scalar_products
    }
}

/// Typed reason why tensor lowering requires a base-field polynomial to be
/// nonzero in addition to the integral family's own domain.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TensorLoweringGuardOrigin {
    InputCoefficientDenominator { input_term: usize },
}

/// One normalized input-coefficient denominator condition with all source
/// tensor terms that contributed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorLoweringNonZeroCondition {
    polynomial: FamilyBasePolynomial,
    origins: BTreeSet<TensorLoweringGuardOrigin>,
}

impl TensorLoweringNonZeroCondition {
    pub fn polynomial(&self) -> &FamilyBasePolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<TensorLoweringGuardOrigin> {
        &self.origins
    }
}

/// Complete exceptional domain of one tensor-lowering result.
///
/// Consumers must carry this object as a unit: `family` authenticates the
/// denominator-coordinate map, while `coefficient_nonzero` retains projector
/// and caller-coefficient denominators such as the `d` in a rank-two vacuum
/// projector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorLoweringDomain {
    family: FamilyDomain,
    coefficient_nonzero: Vec<TensorLoweringNonZeroCondition>,
}

impl TensorLoweringDomain {
    pub const fn family(&self) -> &FamilyDomain {
        &self.family
    }

    pub fn coefficient_nonzero_conditions(&self) -> &[TensorLoweringNonZeroCondition] {
        &self.coefficient_nonzero
    }
}

/// One exact coefficient and the tensor terms from which it was collected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoweredTensorCoefficient {
    coefficient: Coefficient,
    origins: BTreeSet<TensorLoweringOrigin>,
}

impl LoweredTensorCoefficient {
    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn origins(&self) -> &BTreeSet<TensorLoweringOrigin> {
        &self.origins
    }
}

/// Scalar integrals grouped by their remaining free-index metric structure.
///
/// The family fingerprint and full generic domain are retained alongside the
/// terms.  This prevents later rule application from silently composing a
/// numerator expansion with a different denominator map and preserves the
/// provenance carried by the family's nonzero conditions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericTensorIntegralReduction {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    domain: TensorLoweringDomain,
    base_integral: ConcreteIntegralKey,
    source_numerator: GenericTensorNumerator,
    limits: GenericTensorFamilyLimits,
    stats: GenericTensorFamilyStats,
    retained_coefficient_bytes: usize,
    structures: BTreeMap<MetricPairing, BTreeMap<ConcreteIntegralKey, LoweredTensorCoefficient>>,
}

impl GenericTensorIntegralReduction {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn domain(&self) -> &TensorLoweringDomain {
        &self.domain
    }

    pub const fn family_domain(&self) -> &FamilyDomain {
        self.domain.family()
    }

    pub const fn base_integral(&self) -> &ConcreteIntegralKey {
        &self.base_integral
    }

    /// Retained bounded source manifest to which every
    /// [`TensorLoweringOrigin::input_term`] ordinal refers.
    pub const fn source_numerator(&self) -> &GenericTensorNumerator {
        &self.source_numerator
    }

    pub const fn limits(&self) -> GenericTensorFamilyLimits {
        self.limits
    }

    pub const fn stats(&self) -> GenericTensorFamilyStats {
        self.stats
    }

    /// Extra nonzero conditions from tensor input/projector coefficients.
    /// The complete lowering domain is these conditions together with
    /// [`Self::family_domain`].
    pub fn coefficient_nonzero_conditions(&self) -> &[TensorLoweringNonZeroCondition] {
        self.domain.coefficient_nonzero_conditions()
    }

    pub const fn retained_coefficient_bytes(&self) -> usize {
        self.retained_coefficient_bytes
    }

    pub fn structures(
        &self,
    ) -> &BTreeMap<MetricPairing, BTreeMap<ConcreteIntegralKey, LoweredTensorCoefficient>> {
        &self.structures
    }

    pub fn terms_for_structure(
        &self,
        metrics: &MetricPairing,
    ) -> Option<&BTreeMap<ConcreteIntegralKey, LoweredTensorCoefficient>> {
        self.structures.get(metrics)
    }

    pub fn term(
        &self,
        metrics: &MetricPairing,
        integral: &ConcreteIntegralKey,
    ) -> Option<&LoweredTensorCoefficient> {
        self.structures.get(metrics)?.get(integral)
    }

    pub fn is_zero(&self) -> bool {
        self.structures.is_empty()
    }

    pub fn len(&self) -> usize {
        self.structures.values().map(BTreeMap::len).sum()
    }

    /// Replay the complete lowering from the retained source manifest against
    /// the supplied family, including its exact semantic fingerprint and
    /// domain. This is the authentication point for later persistence and
    /// composition with the parametric reduction engine.
    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorFamilyError> {
        let actual: Arc<str> = Arc::from(family.fingerprint());
        if actual != self.family_fingerprint {
            return Err(GenericTensorFamilyError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual,
            });
        }
        let replay = GenericTensorFamilyReducer::with_limits(family, self.limits)
            .lower(&self.base_integral, &self.source_numerator)?;
        if replay == *self {
            Ok(())
        } else {
            Err(GenericTensorFamilyError::InternalVerificationFailure {
                detail: "replayed tensor lowering differs from retained result".to_owned(),
            })
        }
    }
}

/// Exact, bounded scalar-product lowering for one authenticated generic
/// integral family.
#[derive(Clone, Debug)]
pub struct GenericTensorFamilyReducer<'family> {
    family: &'family IntegralFamily,
    family_fingerprint: Arc<str>,
    limits: GenericTensorFamilyLimits,
}

impl<'family> GenericTensorFamilyReducer<'family> {
    pub fn new(family: &'family IntegralFamily) -> Self {
        Self::with_limits(family, GenericTensorFamilyLimits::default())
    }

    pub fn with_limits(family: &'family IntegralFamily, limits: GenericTensorFamilyLimits) -> Self {
        Self {
            family,
            family_fingerprint: Arc::from(family.fingerprint()),
            limits,
        }
    }

    pub const fn family(&self) -> &'family IntegralFamily {
        self.family
    }

    pub const fn limits(&self) -> GenericTensorFamilyLimits {
        self.limits
    }

    /// Lower a generic scalar-product numerator to concrete integral keys.
    pub fn lower(
        &self,
        base_integral: &ConcreteIntegralKey,
        numerator: &GenericTensorNumerator,
    ) -> Result<GenericTensorIntegralReduction, GenericTensorFamilyError> {
        self.validate_base_integral(base_integral)?;
        // Authenticate the caller's exact-algebra policy even for a zero
        // numerator, for which no later Symbolica operation would otherwise
        // observe an invalid configured exponent ceiling.
        self.family.coefficient_context().validate_with_limits(
            &self.family.coefficient_context().one(),
            self.limits.exact_algebra,
        )?;
        check_usize_limit(
            "tensor input terms",
            numerator.terms.len(),
            self.limits.max_input_terms,
        )?;

        let mut structures = BTreeMap::<
            MetricPairing,
            BTreeMap<ConcreteIntegralKey, LoweredTensorCoefficient>,
        >::new();
        let mut output_terms = 0_usize;
        let mut operations = 0_u64;
        let mut source_structure_entries = 0_usize;
        let mut coefficient_nonzero = Vec::<TensorLoweringNonZeroCondition>::new();
        let mut retained_origins = 0_usize;
        let mut nonzero_condition_origins = 0_usize;
        let mut retained_coefficient_bytes = 0_usize;

        for (input_term, term) in numerator.terms.iter().enumerate() {
            self.family
                .coefficient_context()
                .validate_with_limits(term.coefficient(), self.limits.exact_algebra)
                .map_err(|error| GenericTensorFamilyError::InvalidInputCoefficient {
                    input_term,
                    error,
                })?;
            check_usize_limit(
                "tensor distinct scalar products per input",
                term.scalar_products.factors().len(),
                self.limits.max_distinct_scalar_products_per_input,
            )?;
            check_usize_limit(
                "tensor metrics per input",
                term.metrics.metrics().len(),
                self.limits.max_metrics_per_input,
            )?;
            let input_structure_entries = term
                .scalar_products
                .factors()
                .len()
                .checked_add(term.metrics.metrics().len())
                .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "retained tensor source structure entries",
                })?;
            source_structure_entries = source_structure_entries
                .checked_add(input_structure_entries)
                .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "retained tensor source structure entries",
                })?;
            check_usize_limit(
                "retained tensor source structure entries",
                source_structure_entries,
                self.limits.max_source_structure_entries,
            )?;
            charge_coefficient_bytes(
                term.coefficient(),
                &mut retained_coefficient_bytes,
                self.limits.max_retained_coefficient_bytes,
            )?;
            insert_input_denominator_condition(
                term.coefficient(),
                input_term,
                &mut coefficient_nonzero,
                self.limits,
                &mut nonzero_condition_origins,
            )?;
            let degree = term.scalar_products.checked_degree()?;
            if degree > self.limits.max_scalar_product_degree {
                return Err(GenericTensorFamilyError::ScalarProductDegreeLimit {
                    input_term,
                    requested: degree,
                    limit: self.limits.max_scalar_product_degree,
                });
            }
            if term.coefficient().is_zero() {
                continue;
            }
            check_usize_limit(
                "tensor expansion terms per input",
                1,
                self.limits.max_expansion_terms_per_input,
            )?;

            let denominator_count = self.family.denominator_count();
            check_exponent_entry_limit(
                1,
                denominator_count,
                "tensor expansion exponent entries",
                self.limits.max_expansion_exponent_entries,
            )?;
            let mut polynomial = BTreeMap::<Vec<u64>, Coefficient>::from([(
                vec![0; denominator_count],
                term.coefficient().clone(),
            )]);

            for (&coordinate, &exponent) in term.scalar_products.factors() {
                let coordinate_index = self.family.coordinate_index(coordinate)?;
                let expansion = self.family.scalar_product_expansion(coordinate_index)?;
                for _ in 0..exponent {
                    let mut next = BTreeMap::<Vec<u64>, Coefficient>::new();
                    for (shifts, coefficient) in &polynomial {
                        if !expansion.constant().is_zero() {
                            record_operation(
                                &mut operations,
                                self.limits.max_expansion_operations,
                            )?;
                            let product = self.family.coefficient_context().try_mul(
                                coefficient,
                                expansion.constant(),
                                self.limits.exact_algebra,
                            )?;
                            insert_expansion_term(
                                self.family,
                                &mut next,
                                shifts.clone(),
                                product,
                                self.limits,
                                &mut operations,
                            )?;
                        }
                        for (denominator, basis_coefficient) in
                            expansion.denominator_coefficients().iter().enumerate()
                        {
                            if basis_coefficient.is_zero() {
                                continue;
                            }
                            record_operation(
                                &mut operations,
                                self.limits.max_expansion_operations,
                            )?;
                            let mut shifted = shifts.clone();
                            shifted[denominator] = shifted[denominator].checked_add(1).ok_or(
                                GenericTensorFamilyError::IntegralShiftOverflow { denominator },
                            )?;
                            let product = self.family.coefficient_context().try_mul(
                                coefficient,
                                basis_coefficient,
                                self.limits.exact_algebra,
                            )?;
                            insert_expansion_term(
                                self.family,
                                &mut next,
                                shifted,
                                product,
                                self.limits,
                                &mut operations,
                            )?;
                        }
                    }
                    check_exponent_entry_limit(
                        next.len(),
                        self.family.denominator_count(),
                        "tensor expansion exponent entries",
                        self.limits.max_expansion_exponent_entries,
                    )?;
                    polynomial = next;
                    if polynomial.is_empty() {
                        break;
                    }
                }
                if polynomial.is_empty() {
                    break;
                }
            }

            let origin = TensorLoweringOrigin {
                input_term,
                scalar_products: term.scalar_products.clone(),
            };
            for (shifts, coefficient) in polynomial {
                let integral = shifted_integral_key(base_integral, &shifts)?;
                record_operation(&mut operations, self.limits.max_expansion_operations)?;
                insert_output_term(
                    self.family,
                    &mut structures,
                    &mut output_terms,
                    term.metrics.clone(),
                    integral,
                    coefficient,
                    origin.clone(),
                    self.limits,
                    &mut operations,
                    &mut retained_origins,
                    &mut retained_coefficient_bytes,
                )?;
            }
        }

        structures.retain(|_, terms| !terms.is_empty());
        debug_assert_eq!(
            output_terms,
            structures.values().map(BTreeMap::len).sum::<usize>()
        );
        let stats = GenericTensorFamilyStats {
            input_terms: numerator.terms.len(),
            source_structure_entries,
            expansion_operations: operations,
            output_terms,
            output_exponent_entries: output_terms
                .checked_mul(self.family.denominator_count())
                .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "tensor output exponent entries",
                })?,
            retained_origins,
            nonzero_conditions: coefficient_nonzero.len(),
            nonzero_condition_origins,
            retained_coefficient_bytes,
        };
        Ok(GenericTensorIntegralReduction {
            schema: GENERIC_TENSOR_FAMILY_LOWERING_V1_SCHEMA,
            family_fingerprint: self.family_fingerprint.clone(),
            domain: TensorLoweringDomain {
                family: self.family.domain().clone(),
                coefficient_nonzero,
            },
            base_integral: base_integral.clone(),
            source_numerator: numerator.clone(),
            limits: self.limits,
            stats,
            retained_coefficient_bytes,
            structures,
        })
    }

    /// Adapter for the current vacuum projector.  Its loop-vector scalar
    /// products become typed `LoopLoop` family coordinates before entering the
    /// same generic lowering path.  This is an adapter only: the lowering core
    /// itself also supports loop--external coordinates.
    pub fn lower_vacuum_projection(
        &self,
        base_integral: &ConcreteIntegralKey,
        tensor: &TensorReduction,
    ) -> Result<GenericTensorIntegralReduction, GenericTensorFamilyError> {
        if self.family.external_count() != 0 {
            return Err(
                GenericTensorFamilyError::VacuumProjectionNeedsVacuumFamily {
                    externals: self.family.external_count(),
                },
            );
        }
        check_usize_limit(
            "tensor input terms",
            tensor.terms().len(),
            self.limits.max_input_terms,
        )?;
        let mut terms = Vec::with_capacity(tensor.terms().len());
        for term in tensor.terms() {
            let mut scalar_products = GenericScalarProductMonomial::one();
            for (&scalar_product, &exponent) in term.scalar_products().factors() {
                scalar_products.try_multiply_power(
                    ScalarProductCoordinate::LoopLoop {
                        left: usize::from(scalar_product.left().id()),
                        right: usize::from(scalar_product.right().id()),
                    },
                    exponent,
                )?;
            }
            terms.push(GenericTensorTerm::new(
                term.coefficient().clone(),
                term.metrics().clone(),
                scalar_products,
            ));
        }
        let numerator =
            GenericTensorNumerator::try_new_with_limit(terms, self.limits.max_input_terms)?;
        self.lower(base_integral, &numerator)
    }

    fn validate_base_integral(
        &self,
        base_integral: &ConcreteIntegralKey,
    ) -> Result<(), GenericTensorFamilyError> {
        if base_integral.powers().len() != self.family.denominator_count() {
            return Err(GenericTensorFamilyError::WrongIntegralArity {
                expected: self.family.denominator_count(),
                actual: base_integral.powers().len(),
            });
        }
        Ok(())
    }
}

fn insert_expansion_term(
    family: &IntegralFamily,
    polynomial: &mut BTreeMap<Vec<u64>, Coefficient>,
    monomial: Vec<u64>,
    coefficient: Coefficient,
    limits: GenericTensorFamilyLimits,
    operations: &mut u64,
) -> Result<(), GenericTensorFamilyError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    if let Some(current) = polynomial.get(&monomial) {
        record_operation(operations, limits.max_expansion_operations)?;
        let sum =
            family
                .coefficient_context()
                .try_add(current, &coefficient, limits.exact_algebra)?;
        if sum.is_zero() {
            polynomial.remove(&monomial);
        } else {
            polynomial.insert(monomial, sum);
        }
        return Ok(());
    }

    let attempted =
        polynomial
            .len()
            .checked_add(1)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "tensor expansion terms per input",
            })?;
    check_usize_limit(
        "tensor expansion terms per input",
        attempted,
        limits.max_expansion_terms_per_input,
    )?;
    check_exponent_entry_limit(
        attempted,
        family.denominator_count(),
        "tensor expansion exponent entries",
        limits.max_expansion_exponent_entries,
    )?;
    polynomial.insert(monomial, coefficient);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_output_term(
    family: &IntegralFamily,
    structures: &mut BTreeMap<
        MetricPairing,
        BTreeMap<ConcreteIntegralKey, LoweredTensorCoefficient>,
    >,
    output_terms: &mut usize,
    metrics: MetricPairing,
    integral: ConcreteIntegralKey,
    coefficient: Coefficient,
    origin: TensorLoweringOrigin,
    limits: GenericTensorFamilyLimits,
    operations: &mut u64,
    retained_origins: &mut usize,
    retained_coefficient_bytes: &mut usize,
) -> Result<(), GenericTensorFamilyError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    let terms = structures.entry(metrics.clone()).or_default();
    if let Some(current) = terms.get(&integral) {
        record_operation(operations, limits.max_expansion_operations)?;
        let sum = family.coefficient_context().try_add(
            current.coefficient(),
            &coefficient,
            limits.exact_algebra,
        )?;
        if sum.is_zero() {
            terms.remove(&integral);
            *output_terms = output_terms.checked_sub(1).ok_or(
                GenericTensorFamilyError::InternalVerificationFailure {
                    detail: "retained output-term counter underflowed".to_owned(),
                },
            )?;
            if terms.is_empty() {
                structures.remove(&metrics);
            }
        } else {
            let current = terms
                .get_mut(&integral)
                .expect("the checked output term is still present");
            if !current.origins.contains(&origin) {
                let attempted = current.origins.len().checked_add(1).ok_or(
                    GenericTensorFamilyError::ResourceCountOverflow {
                        resource: "tensor output origins",
                    },
                )?;
                check_usize_limit(
                    "tensor output origins",
                    attempted,
                    limits.max_origins_per_output,
                )?;
                let aggregate = retained_origins.checked_add(1).ok_or(
                    GenericTensorFamilyError::ResourceCountOverflow {
                        resource: "retained tensor output origins",
                    },
                )?;
                check_usize_limit(
                    "retained tensor output origins",
                    aggregate,
                    limits.max_retained_origins,
                )?;
                current.origins.insert(origin);
                *retained_origins = aggregate;
            }
            current.coefficient = sum;
            // Recompute the retained byte accounting for this collected
            // coefficient by charging the new value conservatively.  This can
            // over-count replaced values but never under-bounds retained data.
            charge_coefficient_bytes(
                &current.coefficient,
                retained_coefficient_bytes,
                limits.max_retained_coefficient_bytes,
            )?;
        }
        return Ok(());
    }

    let attempted =
        output_terms
            .checked_add(1)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "tensor output terms",
            })?;
    check_usize_limit("tensor output terms", attempted, limits.max_output_terms)?;
    check_exponent_entry_limit(
        attempted,
        family.denominator_count(),
        "tensor output exponent entries",
        limits.max_output_exponent_entries,
    )?;
    check_usize_limit("tensor output origins", 1, limits.max_origins_per_output)?;
    let aggregate =
        retained_origins
            .checked_add(1)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "retained tensor output origins",
            })?;
    check_usize_limit(
        "retained tensor output origins",
        aggregate,
        limits.max_retained_origins,
    )?;
    charge_coefficient_bytes(
        &coefficient,
        retained_coefficient_bytes,
        limits.max_retained_coefficient_bytes,
    )?;
    terms.insert(
        integral,
        LoweredTensorCoefficient {
            coefficient,
            origins: BTreeSet::from([origin]),
        },
    );
    *retained_origins = aggregate;
    *output_terms = attempted;
    Ok(())
}

fn shifted_integral_key(
    base: &ConcreteIntegralKey,
    shifts: &[u64],
) -> Result<ConcreteIntegralKey, GenericTensorFamilyError> {
    if base.powers().len() != shifts.len() {
        return Err(GenericTensorFamilyError::WrongIntegralArity {
            expected: shifts.len(),
            actual: base.powers().len(),
        });
    }
    let powers = base
        .powers()
        .iter()
        .zip(shifts)
        .enumerate()
        .map(|(denominator, (&power, &shift))| {
            let shift = i64::try_from(shift)
                .map_err(|_| GenericTensorFamilyError::IntegralShiftOverflow { denominator })?;
            power
                .checked_sub(shift)
                .ok_or(GenericTensorFamilyError::IntegralPowerOverflow {
                    denominator,
                    power,
                    numerator_power: shift,
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    ConcreteIntegralKey::try_new(powers).map_err(GenericTensorFamilyError::IntegralKey)
}

fn record_operation(operations: &mut u64, limit: u64) -> Result<(), GenericTensorFamilyError> {
    let attempted =
        operations
            .checked_add(1)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "tensor expansion operations",
            })?;
    if attempted > limit {
        return Err(GenericTensorFamilyError::OperationLimit { attempted, limit });
    }
    *operations = attempted;
    Ok(())
}

fn insert_input_denominator_condition(
    coefficient: &Coefficient,
    input_term: usize,
    conditions: &mut Vec<TensorLoweringNonZeroCondition>,
    limits: GenericTensorFamilyLimits,
    aggregate_origins: &mut usize,
) -> Result<(), GenericTensorFamilyError> {
    let polynomial = coefficient.denominator.clone();
    if polynomial.is_constant() && !polynomial.is_zero() {
        return Ok(());
    }
    let origin = TensorLoweringGuardOrigin::InputCoefficientDenominator { input_term };
    if let Some(condition) = conditions
        .iter_mut()
        .find(|condition| condition.polynomial == polynomial)
    {
        if !condition.origins.contains(&origin) {
            let attempted = condition.origins.len().checked_add(1).ok_or(
                GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "tensor nonzero-condition origins",
                },
            )?;
            check_usize_limit(
                "tensor nonzero-condition origins",
                attempted,
                limits.max_origins_per_nonzero_condition,
            )?;
            let aggregate = aggregate_origins.checked_add(1).ok_or(
                GenericTensorFamilyError::ResourceCountOverflow {
                    resource: "retained tensor nonzero-condition origins",
                },
            )?;
            check_usize_limit(
                "retained tensor nonzero-condition origins",
                aggregate,
                limits.max_nonzero_condition_origins,
            )?;
            condition.origins.insert(origin);
            *aggregate_origins = aggregate;
        }
        return Ok(());
    }
    let attempted =
        conditions
            .len()
            .checked_add(1)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "tensor nonzero conditions",
            })?;
    check_usize_limit(
        "tensor nonzero conditions",
        attempted,
        limits.max_nonzero_conditions,
    )?;
    check_usize_limit(
        "tensor nonzero-condition origins",
        1,
        limits.max_origins_per_nonzero_condition,
    )?;
    let aggregate = aggregate_origins.checked_add(1).ok_or(
        GenericTensorFamilyError::ResourceCountOverflow {
            resource: "retained tensor nonzero-condition origins",
        },
    )?;
    check_usize_limit(
        "retained tensor nonzero-condition origins",
        aggregate,
        limits.max_nonzero_condition_origins,
    )?;
    conditions.push(TensorLoweringNonZeroCondition {
        polynomial,
        origins: BTreeSet::from([origin]),
    });
    *aggregate_origins = aggregate;
    Ok(())
}

fn check_exponent_entry_limit(
    terms: usize,
    arity: usize,
    resource: &'static str,
    limit: usize,
) -> Result<(), GenericTensorFamilyError> {
    let requested = terms
        .checked_mul(arity)
        .ok_or(GenericTensorFamilyError::ResourceCountOverflow { resource })?;
    check_usize_limit(resource, requested, limit)
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
    used: &mut usize,
    limit: usize,
) -> Result<(), GenericTensorFamilyError> {
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: limit.saturating_sub(*used),
    };
    write!(&mut writer, "{coefficient}").map_err(|_| GenericTensorFamilyError::ResourceLimit {
        resource: "retained tensor coefficient bytes",
        requested: limit.saturating_add(1),
        limit,
    })?;
    *used =
        used.checked_add(writer.length)
            .ok_or(GenericTensorFamilyError::ResourceCountOverflow {
                resource: "retained tensor coefficient bytes",
            })?;
    Ok(())
}

fn check_usize_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GenericTensorFamilyError> {
    if requested > limit {
        Err(GenericTensorFamilyError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

/// Typed failures from generic tensor-to-family lowering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenericTensorFamilyError {
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    ScalarProductExponentOverflow {
        coordinate: ScalarProductCoordinate,
    },
    ScalarProductDegreeLimit {
        input_term: usize,
        requested: u64,
        limit: u64,
    },
    ConstructorDegreeLimit {
        requested: u64,
        limit: u64,
    },
    IntegralShiftOverflow {
        denominator: usize,
    },
    IntegralPowerOverflow {
        denominator: usize,
        power: i64,
        numerator_power: i64,
    },
    InvalidInputCoefficient {
        input_term: usize,
        error: ExactAlgebraError,
    },
    WrongFamilyFingerprint {
        expected: Arc<str>,
        actual: Arc<str>,
    },
    VacuumProjectionNeedsVacuumFamily {
        externals: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    OperationLimit {
        attempted: u64,
        limit: u64,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    Family(GenericFamilyError),
    ExactAlgebra(ExactAlgebraError),
    IntegralKey(ParametricRelationError),
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for GenericTensorFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "tensor base integral has {actual} powers; expected {expected}"
            ),
            Self::ScalarProductExponentOverflow { coordinate } => write!(
                formatter,
                "the numerator exponent of scalar-product coordinate {coordinate:?} overflows u32"
            ),
            Self::ScalarProductDegreeLimit {
                input_term,
                requested,
                limit,
            } => write!(
                formatter,
                "tensor input term {input_term} has scalar-product degree {requested}, above limit {limit}"
            ),
            Self::ConstructorDegreeLimit { requested, limit } => write!(
                formatter,
                "tensor scalar-product constructor reached degree {requested}, above limit {limit}"
            ),
            Self::IntegralShiftOverflow { denominator } => write!(
                formatter,
                "the numerator-induced shift of denominator {denominator} does not fit in i64"
            ),
            Self::IntegralPowerOverflow {
                denominator,
                power,
                numerator_power,
            } => write!(
                formatter,
                "subtracting numerator power {numerator_power} from integral power {power} overflows at denominator {denominator}"
            ),
            Self::InvalidInputCoefficient { input_term, error } => {
                write!(
                    formatter,
                    "invalid coefficient on tensor input term {input_term}: {error}"
                )
            }
            Self::WrongFamilyFingerprint { expected, actual } => write!(
                formatter,
                "tensor lowering belongs to family fingerprint {expected:?}, not {actual:?}"
            ),
            Self::VacuumProjectionNeedsVacuumFamily { externals } => write!(
                formatter,
                "the vacuum-projector adapter cannot authenticate a family with {externals} external momenta"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::OperationLimit { attempted, limit } => write!(
                formatter,
                "tensor lowering needs at least {attempted} expansion operations, exceeding limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::Family(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::IntegralKey(error) => error.fmt(formatter),
            Self::InternalVerificationFailure { detail } => {
                write!(formatter, "generic tensor lowering replay failed: {detail}")
            }
        }
    }
}

impl Error for GenericTensorFamilyError {}

impl From<GenericFamilyError> for GenericTensorFamilyError {
    fn from(value: GenericFamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<ExactAlgebraError> for GenericTensorFamilyError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}
