//! Bounded, proof-preserving projection and reduction of finite covariant
//! tensor polynomials.
//!
//! LiteRed and Vakint inputs are sums, not isolated monomials.  This module
//! keeps every original weighted source, projects each source through the
//! generic authenticated vacuum projector, collects exact Symbolica
//! coefficients by the *complete* covariant/scalar-product key, and retains
//! all contributions even when a source is odd, has zero weight, or cancels.
//! No loop-count or topology-specific identity appears here.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::generic_family::BasePolynomial;
use crate::tensor_reduction_engine::{
    assemble_covariant_from_witnesses, build_covariant_numerator_lowerings,
};
use crate::{
    AuthenticatedVacuumCovariantTensorProjection, CertifiedRewriteDomainCondition,
    ConcreteIntegralKey, ConcreteRuleProvider, CovariantTensorLoweringStats,
    CovariantTensorMonomial, CovariantTensorParametricReductionResult,
    GenericCovariantTensorNumerator, GenericCovariantTensorTerm, GenericTensorFamilyLimits,
    GenericTensorIntegralReduction, GenericTensorProjectorError, GenericTensorProjectorLimits,
    GenericVacuumTensorProjector, IncompleteTensorReductionError, IntegralFamily,
    ParametricReductionEngine, TensorCovariantStructure, TensorParametricReductionComposer,
    TensorReductionCertificateError, TensorReductionEngineError, TensorReductionGuard,
    algebra::Coefficient, algebra::ExactAlgebraError, algebra::ExactAlgebraLimits,
};

pub const GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V1_SCHEMA: &str =
    "rustred-generic-vacuum-covariant-tensor-polynomial-projection-v1";
pub const GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V2_SCHEMA: &str =
    "rustred-generic-vacuum-covariant-tensor-polynomial-projection-v2";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_LOWERING_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-polynomial-lowering-v1";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_LOWERING_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-polynomial-lowering-v2";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-polynomial-parametric-reduction-v1";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-polynomial-parametric-reduction-v2";

/// Aggregate policy for one complete finite tensor-polynomial projection.
///
/// `projector` supplies the per-source shape ceilings. Every work/retention
/// field below is shared across all source projections and collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericTensorPolynomialLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub projector: GenericTensorProjectorLimits,
    pub max_source_terms: usize,
    pub max_source_structure_entries: usize,
    pub max_family_domain_copies: usize,
    pub max_family_domain_conditions: usize,
    pub max_family_domain_origins: usize,
    pub max_family_domain_polynomial_terms: usize,
    pub max_family_domain_exponent_entries: usize,
    pub max_family_manifest_bytes: usize,
    pub max_projection_arithmetic_operations: u64,
    pub max_projection_matrix_peak_live_entries: usize,
    pub max_projection_matrix_input_retained_bytes: usize,
    pub max_projection_matrix_output_retained_bytes: usize,
    pub max_projection_structural_operations: u64,
    pub max_projection_pairings: usize,
    pub max_projection_gram_entries: usize,
    pub max_projection_augmented_entries: usize,
    pub max_projection_inverse_entries: usize,
    pub max_projection_candidates: usize,
    pub max_projected_terms: usize,
    pub max_projected_structure_entries: usize,
    pub max_projection_nonzero_conditions: usize,
    pub max_projection_guard_origins: usize,
    pub max_projection_guard_polynomial_terms: usize,
    pub max_projection_guard_exponent_entries: usize,
    pub max_weight_nonzero_conditions: usize,
    pub max_weight_guard_origins: usize,
    pub max_weight_guard_polynomial_terms: usize,
    pub max_weight_guard_exponent_entries: usize,
    pub max_contributions: usize,
    pub max_collection_operations: u64,
    pub max_collected_terms: usize,
    pub max_origins_per_monomial: usize,
    /// Counts every retained clone of a source or complete
    /// `(TensorCovariantStructure, loop scalar monomial)` key.
    pub max_retained_structure_entries: usize,
    /// Bounded Debug bytes for the same retained source/key clones.
    pub max_retained_structure_bytes: usize,
    /// Conservative formatted bytes of weights, projector coefficients,
    /// contributions, collection updates, and final numerator coefficients.
    pub max_retained_coefficient_bytes: usize,
    pub max_retained_guard_bytes: usize,
}

impl Default for GenericTensorPolynomialLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            projector: GenericTensorProjectorLimits::default(),
            max_source_terms: 100_000,
            max_source_structure_entries: 16_000_000,
            max_family_domain_copies: 100_000,
            max_family_domain_conditions: 10_000_000,
            max_family_domain_origins: 1_000_000_000,
            max_family_domain_polynomial_terms: 100_000_000,
            max_family_domain_exponent_entries: 1_000_000_000,
            max_family_manifest_bytes: 256 * 1024 * 1024,
            max_projection_arithmetic_operations: 1_000_000_000,
            max_projection_matrix_peak_live_entries: 4_000_000,
            max_projection_matrix_input_retained_bytes: 1024 * 1024 * 1024,
            max_projection_matrix_output_retained_bytes: 1024 * 1024 * 1024,
            max_projection_structural_operations: 1_000_000_000,
            max_projection_pairings: 10_000_000,
            max_projection_gram_entries: 100_000_000,
            max_projection_augmented_entries: 200_000_000,
            max_projection_inverse_entries: 100_000_000,
            max_projection_candidates: 100_000_000,
            max_projected_terms: 100_000_000,
            max_projected_structure_entries: 1_000_000_000,
            max_projection_nonzero_conditions: 10_000_000,
            max_projection_guard_origins: 100_000_000,
            max_projection_guard_polynomial_terms: 100_000_000,
            max_projection_guard_exponent_entries: 1_000_000_000,
            max_weight_nonzero_conditions: 1_000_000,
            max_weight_guard_origins: 10_000_000,
            max_weight_guard_polynomial_terms: 16_000_000,
            max_weight_guard_exponent_entries: 64_000_000,
            max_contributions: 100_000_000,
            max_collection_operations: 1_000_000_000,
            max_collected_terms: 100_000_000,
            max_origins_per_monomial: 10_000_000,
            max_retained_structure_entries: 1_000_000_000,
            max_retained_structure_bytes: 512 * 1024 * 1024,
            max_retained_coefficient_bytes: 1024 * 1024 * 1024,
            max_retained_guard_bytes: 256 * 1024 * 1024,
        }
    }
}

/// One coefficient-weighted original covariant tensor monomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeightedCovariantTensorMonomial {
    coefficient: Coefficient,
    monomial: CovariantTensorMonomial,
}

impl WeightedCovariantTensorMonomial {
    pub fn new(coefficient: Coefficient, monomial: CovariantTensorMonomial) -> Self {
        Self {
            coefficient,
            monomial,
        }
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub const fn monomial(&self) -> &CovariantTensorMonomial {
        &self.monomial
    }
}

/// Full key used for exact sum-level collection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CovariantTensorPolynomialMonomial {
    covariant: TensorCovariantStructure,
    loop_scalar_products: crate::GenericScalarProductMonomial,
}

impl CovariantTensorPolynomialMonomial {
    pub const fn covariant(&self) -> &TensorCovariantStructure {
        &self.covariant
    }

    pub const fn loop_scalar_products(&self) -> &crate::GenericScalarProductMonomial {
        &self.loop_scalar_products
    }
}

/// Stable address of one projector output inside one original source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TensorPolynomialProjectionOrigin {
    source_term: usize,
    projected_term: usize,
}

impl TensorPolynomialProjectionOrigin {
    pub const fn source_term(self) -> usize {
        self.source_term
    }

    pub const fn projected_term(self) -> usize {
        self.projected_term
    }
}

/// One uncollected weighted projector contribution. Zero-weight
/// contributions are retained; odd projections have no contribution but are
/// retained in `source_projections`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorPolynomialProjectionContribution {
    origin: TensorPolynomialProjectionOrigin,
    monomial: CovariantTensorPolynomialMonomial,
    coefficient: Coefficient,
}

impl TensorPolynomialProjectionContribution {
    pub const fn origin(&self) -> TensorPolynomialProjectionOrigin {
        self.origin
    }

    pub const fn monomial(&self) -> &CovariantTensorPolynomialMonomial {
        &self.monomial
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TensorPolynomialWeightGuardOrigin {
    source_term: usize,
}

impl TensorPolynomialWeightGuardOrigin {
    pub const fn source_term(self) -> usize {
        self.source_term
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorPolynomialWeightNonZeroCondition {
    polynomial: BasePolynomial,
    origins: BTreeSet<TensorPolynomialWeightGuardOrigin>,
}

impl TensorPolynomialWeightNonZeroCondition {
    pub const fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    pub const fn origins(&self) -> &BTreeSet<TensorPolynomialWeightGuardOrigin> {
        &self.origins
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GenericTensorPolynomialStats {
    pub source_terms: usize,
    pub source_structure_entries: usize,
    pub projected_terms: usize,
    pub projection_arithmetic_operations: u64,
    pub projection_symbolica_algebra_operations: u64,
    pub projection_matrix_peak_live_entries: usize,
    pub projection_matrix_input_retained_bytes: usize,
    pub projection_matrix_output_retained_bytes: usize,
    pub projection_structural_operations: u64,
    pub projection_pairings: usize,
    pub projection_gram_entries: usize,
    pub projection_augmented_entries: usize,
    pub projection_inverse_entries: usize,
    pub projection_candidates: usize,
    pub projected_structure_entries: usize,
    pub projection_nonzero_conditions: usize,
    pub projection_guard_origins: usize,
    pub family_domain_origins: usize,
    pub projection_guard_polynomial_terms: usize,
    pub projection_guard_exponent_entries: usize,
    pub weight_nonzero_conditions: usize,
    pub weight_guard_origins: usize,
    pub contributions: usize,
    pub collection_operations: u64,
    pub collected_terms: usize,
    pub retained_structure_entries: usize,
    pub retained_structure_bytes: usize,
    pub retained_coefficient_bytes: usize,
    pub retained_guard_bytes: usize,
}

/// Authenticated projection of a complete finite tensor polynomial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorPolynomialProjection {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    limits: GenericTensorPolynomialLimits,
    sources: Vec<WeightedCovariantTensorMonomial>,
    source_projections: Vec<AuthenticatedVacuumCovariantTensorProjection>,
    weight_nonzero_conditions: Vec<TensorPolynomialWeightNonZeroCondition>,
    contributions: Vec<TensorPolynomialProjectionContribution>,
    provenance:
        BTreeMap<CovariantTensorPolynomialMonomial, BTreeSet<TensorPolynomialProjectionOrigin>>,
    numerator: GenericCovariantTensorNumerator,
    output_origins: Vec<BTreeSet<TensorPolynomialProjectionOrigin>>,
    stats: GenericTensorPolynomialStats,
}

impl AuthenticatedVacuumCovariantTensorPolynomialProjection {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn limits(&self) -> GenericTensorPolynomialLimits {
        self.limits
    }

    pub fn sources(&self) -> &[WeightedCovariantTensorMonomial] {
        &self.sources
    }

    pub fn source_projections(&self) -> &[AuthenticatedVacuumCovariantTensorProjection] {
        &self.source_projections
    }

    pub fn source_projection(
        &self,
        source_term: usize,
    ) -> Option<&AuthenticatedVacuumCovariantTensorProjection> {
        self.source_projections.get(source_term)
    }

    pub fn weight_nonzero_conditions(&self) -> &[TensorPolynomialWeightNonZeroCondition] {
        &self.weight_nonzero_conditions
    }

    pub fn contributions(&self) -> &[TensorPolynomialProjectionContribution] {
        &self.contributions
    }

    /// Provenance is retained for every projected monomial, including keys
    /// whose exact coefficient cancels to zero.
    pub const fn provenance(
        &self,
    ) -> &BTreeMap<CovariantTensorPolynomialMonomial, BTreeSet<TensorPolynomialProjectionOrigin>>
    {
        &self.provenance
    }

    pub const fn numerator(&self) -> &GenericCovariantTensorNumerator {
        &self.numerator
    }

    pub fn output_origins(
        &self,
        output_term: usize,
    ) -> Option<&BTreeSet<TensorPolynomialProjectionOrigin>> {
        self.output_origins.get(output_term)
    }

    pub const fn stats(&self) -> GenericTensorPolynomialStats {
        self.stats
    }

    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorPolynomialError> {
        let actual: Arc<str> = Arc::from(family.fingerprint());
        if actual != self.family_fingerprint {
            return Err(GenericTensorPolynomialError::WrongFamilyFingerprint {
                expected: self.family_fingerprint.clone(),
                actual,
            });
        }
        let replay = GenericVacuumTensorPolynomialProjector::with_limits(self.limits)
            .project(family, self.sources.clone())?;
        if replay == *self {
            Ok(())
        } else {
            Err(GenericTensorPolynomialError::ProjectionReplayMismatch)
        }
    }

    pub fn lower(
        self,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
    ) -> Result<AuthenticatedVacuumCovariantTensorPolynomialLowering, GenericTensorPolynomialError>
    {
        self.lower_with_limits(family, base_integral, GenericTensorFamilyLimits::default())
    }

    pub fn lower_with_limits(
        self,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
        limits: GenericTensorFamilyLimits,
    ) -> Result<AuthenticatedVacuumCovariantTensorPolynomialLowering, GenericTensorPolynomialError>
    {
        AuthenticatedVacuumCovariantTensorPolynomialLowering::try_new_with_limits(
            self,
            family,
            base_integral,
            limits,
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenericVacuumTensorPolynomialProjector {
    limits: GenericTensorPolynomialLimits,
}

impl Default for GenericVacuumTensorPolynomialProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericVacuumTensorPolynomialProjector {
    pub fn new() -> Self {
        Self::with_limits(GenericTensorPolynomialLimits::default())
    }

    pub const fn with_limits(limits: GenericTensorPolynomialLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(&self) -> GenericTensorPolynomialLimits {
        self.limits
    }

    pub fn project(
        &self,
        family: &IntegralFamily,
        sources: impl IntoIterator<Item = WeightedCovariantTensorMonomial>,
    ) -> Result<AuthenticatedVacuumCovariantTensorPolynomialProjection, GenericTensorPolynomialError>
    {
        let context = family.coefficient_context();
        context.validate_with_limits(&context.one(), self.limits.exact_algebra)?;
        let mut retained_sources = Vec::new();
        let mut source_structure_entries = 0_usize;
        let mut retained_structure_entries = 0_usize;
        let mut retained_structure_bytes = 0_usize;
        let mut retained_coefficient_bytes = 0_usize;
        for source in sources {
            let attempted =
                checked_add(retained_sources.len(), 1, "tensor polynomial source terms")?;
            check_limit(
                "tensor polynomial source terms",
                attempted,
                self.limits.max_source_terms,
            )?;
            context.validate_with_limits(source.coefficient(), self.limits.exact_algebra)?;
            let entries = source_entries(source.monomial())?;
            source_structure_entries = checked_add(
                source_structure_entries,
                entries,
                "tensor polynomial source structure entries",
            )?;
            check_limit(
                "tensor polynomial source structure entries",
                source_structure_entries,
                self.limits.max_source_structure_entries,
            )?;
            // One clone is retained in the source manifest and one in the
            // independently replayable child projection.
            charge_structure(
                source.monomial(),
                entries,
                2,
                &mut retained_structure_entries,
                &mut retained_structure_bytes,
                self.limits,
            )?;
            charge_coefficient(
                source.coefficient(),
                &mut retained_coefficient_bytes,
                self.limits.max_retained_coefficient_bytes,
            )?;
            retained_sources.push(source);
        }

        preflight_family_copies(family, retained_sources.len(), self.limits)?;

        let mut source_projections = Vec::new();
        let mut weight_nonzero_conditions = Vec::new();
        let mut contributions = Vec::new();
        let mut provenance = BTreeMap::<
            CovariantTensorPolynomialMonomial,
            BTreeSet<TensorPolynomialProjectionOrigin>,
        >::new();
        let mut collected = BTreeMap::<CovariantTensorPolynomialMonomial, Coefficient>::new();
        let mut stats = GenericTensorPolynomialStats {
            source_terms: retained_sources.len(),
            source_structure_entries,
            retained_structure_entries,
            retained_structure_bytes,
            retained_coefficient_bytes,
            ..GenericTensorPolynomialStats::default()
        };

        for (source_term, source) in retained_sources.iter().enumerate() {
            insert_weight_guard(
                source.coefficient(),
                source_term,
                &mut weight_nonzero_conditions,
                &mut stats,
                self.limits,
            )?;
            let child_limits = remaining_projector_limits(self.limits, stats)?;
            let projection = GenericVacuumTensorProjector::with_limits(child_limits)
                .project_covariant(family, source.monomial())?;
            absorb_projection_stats(&projection, &mut stats, self.limits)?;

            for (projected_term, term) in projection.numerator().terms().iter().enumerate() {
                record_collection_operation(&mut stats, self.limits)?;
                let coefficient = context.try_mul(
                    source.coefficient(),
                    term.coefficient(),
                    self.limits.exact_algebra,
                )?;
                charge_coefficient(
                    &coefficient,
                    &mut stats.retained_coefficient_bytes,
                    self.limits.max_retained_coefficient_bytes,
                )?;
                let monomial = CovariantTensorPolynomialMonomial {
                    covariant: term.covariant().clone(),
                    loop_scalar_products: term.loop_scalar_products().clone(),
                };
                let entries = polynomial_monomial_entries(&monomial)?;
                let origin = TensorPolynomialProjectionOrigin {
                    source_term,
                    projected_term,
                };

                check_limit(
                    "tensor polynomial contributions",
                    checked_add(contributions.len(), 1, "tensor polynomial contributions")?,
                    self.limits.max_contributions,
                )?;
                charge_structure(
                    &monomial,
                    entries,
                    1,
                    &mut stats.retained_structure_entries,
                    &mut stats.retained_structure_bytes,
                    self.limits,
                )?;
                contributions.push(TensorPolynomialProjectionContribution {
                    origin,
                    monomial: monomial.clone(),
                    coefficient: coefficient.clone(),
                });

                if !provenance.contains_key(&monomial) {
                    charge_structure(
                        &monomial,
                        entries,
                        1,
                        &mut stats.retained_structure_entries,
                        &mut stats.retained_structure_bytes,
                        self.limits,
                    )?;
                }
                let origins = provenance.entry(monomial.clone()).or_default();
                if origins.insert(origin) {
                    check_limit(
                        "tensor polynomial origins per monomial",
                        origins.len(),
                        self.limits.max_origins_per_monomial,
                    )?;
                }

                if coefficient.is_zero() {
                    continue;
                }
                if let Some(current) = collected.get(&monomial) {
                    record_collection_operation(&mut stats, self.limits)?;
                    let sum = context.try_add(current, &coefficient, self.limits.exact_algebra)?;
                    charge_coefficient(
                        &sum,
                        &mut stats.retained_coefficient_bytes,
                        self.limits.max_retained_coefficient_bytes,
                    )?;
                    if sum.is_zero() {
                        collected.remove(&monomial);
                    } else {
                        collected.insert(monomial, sum);
                    }
                } else {
                    let attempted =
                        checked_add(collected.len(), 1, "tensor polynomial collected terms")?;
                    check_limit(
                        "tensor polynomial collected terms",
                        attempted,
                        self.limits.max_collected_terms,
                    )?;
                    charge_structure(
                        &monomial,
                        entries,
                        1,
                        &mut stats.retained_structure_entries,
                        &mut stats.retained_structure_bytes,
                        self.limits,
                    )?;
                    collected.insert(monomial, coefficient);
                }
            }
            source_projections.push(projection);
        }

        let mut terms = Vec::new();
        let mut output_origins = Vec::new();
        for (monomial, coefficient) in collected {
            let entries = polynomial_monomial_entries(&monomial)?;
            // The final generic numerator owns another complete key clone.
            charge_structure(
                &monomial,
                entries,
                1,
                &mut stats.retained_structure_entries,
                &mut stats.retained_structure_bytes,
                self.limits,
            )?;
            charge_coefficient(
                &coefficient,
                &mut stats.retained_coefficient_bytes,
                self.limits.max_retained_coefficient_bytes,
            )?;
            output_origins.push(provenance.get(&monomial).cloned().ok_or(
                GenericTensorPolynomialError::InternalVerificationFailure {
                    detail: "surviving collected monomial has no source provenance".to_owned(),
                },
            )?);
            terms.push(GenericCovariantTensorTerm::new(
                coefficient,
                monomial.covariant,
                monomial.loop_scalar_products,
            ));
        }
        stats.weight_nonzero_conditions = weight_nonzero_conditions.len();
        stats.contributions = contributions.len();
        stats.collected_terms = terms.len();
        let numerator = GenericCovariantTensorNumerator::try_new_with_limit(
            terms,
            self.limits.max_collected_terms,
        )?;
        Ok(AuthenticatedVacuumCovariantTensorPolynomialProjection {
            schema: GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V2_SCHEMA,
            family_fingerprint: Arc::from(family.fingerprint()),
            limits: self.limits,
            sources: retained_sources,
            source_projections,
            weight_nonzero_conditions,
            contributions,
            provenance,
            numerator,
            output_origins,
            stats,
        })
    }
}

/// Projection-bound aggregate family lowering. Each covariant lowering is
/// individually replayable and `stats` spends one shared budget across all
/// covariants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorPolynomialLowering {
    schema: &'static str,
    projection: AuthenticatedVacuumCovariantTensorPolynomialProjection,
    base_integral: ConcreteIntegralKey,
    lowering_limits: GenericTensorFamilyLimits,
    stats: CovariantTensorLoweringStats,
    lowerings: BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
}

impl AuthenticatedVacuumCovariantTensorPolynomialLowering {
    pub fn try_new(
        projection: AuthenticatedVacuumCovariantTensorPolynomialProjection,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
    ) -> Result<Self, GenericTensorPolynomialError> {
        Self::try_new_with_limits(
            projection,
            family,
            base_integral,
            GenericTensorFamilyLimits::default(),
        )
    }

    pub fn try_new_with_limits(
        projection: AuthenticatedVacuumCovariantTensorPolynomialProjection,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
        lowering_limits: GenericTensorFamilyLimits,
    ) -> Result<Self, GenericTensorPolynomialError> {
        projection.verify(family)?;
        let (lowerings, stats) = build_covariant_numerator_lowerings(
            projection.numerator(),
            family,
            base_integral,
            lowering_limits,
        )?;
        Ok(Self {
            schema: AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_LOWERING_V2_SCHEMA,
            projection,
            base_integral: base_integral.clone(),
            lowering_limits,
            stats,
            lowerings,
        })
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn projection(&self) -> &AuthenticatedVacuumCovariantTensorPolynomialProjection {
        &self.projection
    }

    pub const fn base_integral(&self) -> &ConcreteIntegralKey {
        &self.base_integral
    }

    pub const fn lowering_limits(&self) -> GenericTensorFamilyLimits {
        self.lowering_limits
    }

    pub const fn stats(&self) -> CovariantTensorLoweringStats {
        self.stats
    }

    pub const fn lowerings(
        &self,
    ) -> &BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction> {
        &self.lowerings
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorPolynomialError> {
        self.projection.verify(family)?;
        let (replay, stats) = build_covariant_numerator_lowerings(
            self.projection.numerator(),
            family,
            &self.base_integral,
            self.lowering_limits,
        )?;
        if replay == self.lowerings && stats == self.stats {
            Ok(())
        } else {
            Err(GenericTensorPolynomialError::LoweringReplayMismatch)
        }
    }

    /// Resolve a grouped lowering origin back to all original polynomial
    /// sources contributing to that collected numerator term.
    pub fn polynomial_origins_for_lowering_origin(
        &self,
        covariant: &TensorCovariantStructure,
        grouped_input_term: usize,
    ) -> Option<&BTreeSet<TensorPolynomialProjectionOrigin>> {
        let output_term = self
            .projection
            .numerator()
            .terms()
            .iter()
            .enumerate()
            .filter(|(_, term)| term.covariant() == covariant)
            .nth(grouped_input_term)?
            .0;
        self.projection.output_origins(output_term)
    }
}

/// End-to-end sum certificate retaining original sources, all projections,
/// exact collection provenance, family lowerings, scalar witnesses, guards,
/// and terminal classifications.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorPolynomialParametricReduction {
    schema: &'static str,
    authenticated_lowering: AuthenticatedVacuumCovariantTensorPolynomialLowering,
    scalar_reduction: CovariantTensorParametricReductionResult,
}

impl AuthenticatedVacuumCovariantTensorPolynomialParametricReduction {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn authenticated_lowering(
        &self,
    ) -> &AuthenticatedVacuumCovariantTensorPolynomialLowering {
        &self.authenticated_lowering
    }

    pub const fn projection(&self) -> &AuthenticatedVacuumCovariantTensorPolynomialProjection {
        self.authenticated_lowering.projection()
    }

    pub const fn scalar_reduction(&self) -> &CovariantTensorParametricReductionResult {
        &self.scalar_reduction
    }

    pub fn scalar_guards(&self) -> &[TensorReductionGuard] {
        self.scalar_reduction.guards()
    }

    /// Certified zero-sector/symmetry/elimination conditions, keyed by the
    /// scalar source that required them.  These are deliberately separate
    /// from parametric specialization guards returned by [`Self::scalar_guards`].
    pub fn scalar_certified_domains(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ConcreteIntegralKey, &[CertifiedRewriteDomainCondition])>
    {
        self.scalar_reduction
            .scalar_witnesses()
            .iter()
            .map(|(source, witness)| (source, witness.certified_domain()))
    }

    pub fn scalar_certified_domain(
        &self,
        source: &ConcreteIntegralKey,
    ) -> Option<&[CertifiedRewriteDomainCondition]> {
        self.scalar_reduction
            .scalar_witnesses()
            .get(source)
            .map(crate::ScalarReductionWitness::certified_domain)
    }

    pub fn require_complete(&self) -> Result<&Self, IncompleteTensorReductionError> {
        self.scalar_reduction.require_complete()?;
        Ok(self)
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), GenericTensorPolynomialError> {
        self.authenticated_lowering.verify(family)?;
        let replay = assemble_covariant_from_witnesses(
            family,
            self.authenticated_lowering.lowerings(),
            self.scalar_reduction.scalar_witnesses(),
            self.scalar_reduction.limits(),
        )?;
        if replay == self.scalar_reduction {
            Ok(())
        } else {
            Err(GenericTensorPolynomialError::ParametricReplayMismatch)
        }
    }

    pub fn verify_with_engine<Provider>(
        &self,
        family: &IntegralFamily,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<(), TensorPolynomialReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        self.authenticated_lowering.verify(family)?;
        let replay =
            TensorParametricReductionComposer::with_limits(family, self.scalar_reduction.limits())
                .reduce_covariant_lowerings(self.authenticated_lowering.lowerings(), engine)?;
        if replay == self.scalar_reduction {
            Ok(())
        } else {
            Err(GenericTensorPolynomialError::ParametricReplayMismatch.into())
        }
    }
}

impl<'family> TensorParametricReductionComposer<'family> {
    pub fn reduce_authenticated_covariant_polynomial<Provider>(
        &self,
        authenticated_lowering: AuthenticatedVacuumCovariantTensorPolynomialLowering,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<
        AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
        TensorPolynomialReductionEngineError<Provider::Error>,
    >
    where
        Provider: ConcreteRuleProvider,
    {
        authenticated_lowering.verify(self.family())?;
        let scalar_reduction =
            self.reduce_covariant_lowerings(authenticated_lowering.lowerings(), engine)?;
        Ok(
            AuthenticatedVacuumCovariantTensorPolynomialParametricReduction {
                schema:
                    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V2_SCHEMA,
                authenticated_lowering,
                scalar_reduction,
            },
        )
    }
}

fn remaining_projector_limits(
    limits: GenericTensorPolynomialLimits,
    stats: GenericTensorPolynomialStats,
) -> Result<GenericTensorProjectorLimits, GenericTensorPolynomialError> {
    let mut child = limits.projector;
    child.exact_algebra = limits.exact_algebra;
    child.max_arithmetic_operations = child.max_arithmetic_operations.min(
        limits
            .max_projection_arithmetic_operations
            .saturating_sub(stats.projection_arithmetic_operations),
    );
    child.max_structural_operations = child.max_structural_operations.min(
        limits
            .max_projection_structural_operations
            .saturating_sub(stats.projection_structural_operations),
    );
    child.max_matrix_input_retained_bytes = child.max_matrix_input_retained_bytes.min(
        limits
            .max_projection_matrix_input_retained_bytes
            .saturating_sub(stats.projection_matrix_input_retained_bytes),
    );
    child.max_matrix_live_entries = child
        .max_matrix_live_entries
        .min(limits.max_projection_matrix_peak_live_entries);
    child.max_matrix_output_retained_bytes = child.max_matrix_output_retained_bytes.min(
        limits
            .max_projection_matrix_output_retained_bytes
            .saturating_sub(stats.projection_matrix_output_retained_bytes),
    );
    child.max_pairings = child.max_pairings.min(
        limits
            .max_projection_pairings
            .saturating_sub(stats.projection_pairings),
    );
    child.max_gram_entries = child.max_gram_entries.min(
        limits
            .max_projection_gram_entries
            .saturating_sub(stats.projection_gram_entries)
            .min(
                limits
                    .max_projection_inverse_entries
                    .saturating_sub(stats.projection_inverse_entries),
            ),
    );
    child.max_augmented_entries = child.max_augmented_entries.min(
        limits
            .max_projection_augmented_entries
            .saturating_sub(stats.projection_augmented_entries),
    );
    child.max_projection_candidates = child.max_projection_candidates.min(
        limits
            .max_projection_candidates
            .saturating_sub(stats.projection_candidates),
    );
    child.max_output_terms = child.max_output_terms.min(
        limits
            .max_projected_terms
            .saturating_sub(stats.projected_terms),
    );
    child.max_output_structure_entries = child.max_output_structure_entries.min(
        limits
            .max_projected_structure_entries
            .saturating_sub(stats.projected_structure_entries),
    );
    child.max_nonzero_conditions = child.max_nonzero_conditions.min(
        limits
            .max_projection_nonzero_conditions
            .saturating_sub(stats.projection_nonzero_conditions),
    );
    child.max_guard_origins = child.max_guard_origins.min(
        limits
            .max_projection_guard_origins
            .saturating_sub(stats.projection_guard_origins),
    );
    child.max_guard_polynomial_terms = child.max_guard_polynomial_terms.min(
        limits
            .max_projection_guard_polynomial_terms
            .saturating_sub(stats.projection_guard_polynomial_terms),
    );
    child.max_guard_exponent_entries = child.max_guard_exponent_entries.min(
        limits
            .max_projection_guard_exponent_entries
            .saturating_sub(stats.projection_guard_exponent_entries),
    );
    child.max_family_domain_origins = child.max_family_domain_origins.min(
        limits
            .max_family_domain_origins
            .saturating_sub(stats.family_domain_origins),
    );
    child.max_retained_coefficient_bytes = child.max_retained_coefficient_bytes.min(
        limits
            .max_retained_coefficient_bytes
            .saturating_sub(stats.retained_coefficient_bytes),
    );
    Ok(child)
}

fn absorb_projection_stats(
    projection: &AuthenticatedVacuumCovariantTensorProjection,
    aggregate: &mut GenericTensorPolynomialStats,
    limits: GenericTensorPolynomialLimits,
) -> Result<(), GenericTensorPolynomialError> {
    let stats = projection.stats();
    aggregate.projected_terms = checked_add(
        aggregate.projected_terms,
        stats.output_terms,
        "tensor polynomial projected terms",
    )?;
    check_limit(
        "tensor polynomial projected terms",
        aggregate.projected_terms,
        limits.max_projected_terms,
    )?;
    aggregate.projection_arithmetic_operations = checked_add_u64(
        aggregate.projection_arithmetic_operations,
        stats.arithmetic_operations,
        "tensor polynomial projection arithmetic operations",
    )?;
    check_limit_u64(
        "tensor polynomial projection arithmetic operations",
        aggregate.projection_arithmetic_operations,
        limits.max_projection_arithmetic_operations,
    )?;
    aggregate.projection_symbolica_algebra_operations = checked_add_u64(
        aggregate.projection_symbolica_algebra_operations,
        stats.symbolica_algebra_operations,
        "tensor polynomial Symbolica algebra operations",
    )?;
    aggregate.projection_matrix_peak_live_entries = aggregate
        .projection_matrix_peak_live_entries
        .max(stats.matrix_peak_live_entries);
    check_limit(
        "tensor polynomial Symbolica matrix peak live entries",
        aggregate.projection_matrix_peak_live_entries,
        limits.max_projection_matrix_peak_live_entries,
    )?;
    aggregate.projection_structural_operations = checked_add_u64(
        aggregate.projection_structural_operations,
        stats.structural_operations,
        "tensor polynomial projection structural operations",
    )?;
    check_limit_u64(
        "tensor polynomial projection structural operations",
        aggregate.projection_structural_operations,
        limits.max_projection_structural_operations,
    )?;
    macro_rules! absorb {
        ($field:ident, $value:expr, $resource:literal, $limit:expr) => {{
            aggregate.$field = checked_add(aggregate.$field, $value, $resource)?;
            check_limit($resource, aggregate.$field, $limit)?;
        }};
    }
    absorb!(
        projection_matrix_input_retained_bytes,
        stats.matrix_input_retained_bytes,
        "tensor polynomial Symbolica matrix input retained bytes",
        limits.max_projection_matrix_input_retained_bytes
    );
    absorb!(
        projection_matrix_output_retained_bytes,
        stats.matrix_output_retained_bytes,
        "tensor polynomial Symbolica matrix output retained bytes",
        limits.max_projection_matrix_output_retained_bytes
    );
    absorb!(
        projection_pairings,
        stats.pairing_count,
        "tensor polynomial projection pairings",
        limits.max_projection_pairings
    );
    absorb!(
        projection_gram_entries,
        stats.gram_entries,
        "tensor polynomial projection Gram entries",
        limits.max_projection_gram_entries
    );
    let augmented = stats.gram_entries.checked_mul(2).ok_or(
        GenericTensorPolynomialError::ResourceCountOverflow {
            resource: "tensor polynomial projection augmented entries",
        },
    )?;
    absorb!(
        projection_augmented_entries,
        augmented,
        "tensor polynomial projection augmented entries",
        limits.max_projection_augmented_entries
    );
    absorb!(
        projection_inverse_entries,
        stats.inverse_entries,
        "tensor polynomial projection inverse entries",
        limits.max_projection_inverse_entries
    );
    absorb!(
        projection_candidates,
        stats.projection_candidates,
        "tensor polynomial projection candidates",
        limits.max_projection_candidates
    );
    absorb!(
        projected_structure_entries,
        stats.consumed_output_structure_entries,
        "tensor polynomial projected structure entries",
        limits.max_projected_structure_entries
    );
    absorb!(
        projection_guard_polynomial_terms,
        stats.guard_polynomial_terms,
        "tensor polynomial projection guard polynomial terms",
        limits.max_projection_guard_polynomial_terms
    );
    absorb!(
        projection_guard_exponent_entries,
        stats.guard_exponent_entries,
        "tensor polynomial projection guard exponent entries",
        limits.max_projection_guard_exponent_entries
    );
    absorb!(
        projection_guard_origins,
        stats.guard_origins,
        "tensor polynomial projection guard origins",
        limits.max_projection_guard_origins
    );
    absorb!(
        family_domain_origins,
        stats.family_domain_origins,
        "tensor polynomial family-domain origins",
        limits.max_family_domain_origins
    );
    absorb!(
        retained_coefficient_bytes,
        stats.retained_coefficient_bytes,
        "tensor polynomial retained coefficient bytes",
        limits.max_retained_coefficient_bytes
    );
    let conditions = projection.domain().projection_nonzero_conditions().len();
    absorb!(
        projection_nonzero_conditions,
        conditions,
        "tensor polynomial projection nonzero conditions",
        limits.max_projection_nonzero_conditions
    );
    Ok(())
}

fn insert_weight_guard(
    coefficient: &Coefficient,
    source_term: usize,
    conditions: &mut Vec<TensorPolynomialWeightNonZeroCondition>,
    stats: &mut GenericTensorPolynomialStats,
    limits: GenericTensorPolynomialLimits,
) -> Result<(), GenericTensorPolynomialError> {
    let polynomial = coefficient.denominator.clone();
    if polynomial.is_constant() && !polynomial.is_zero() {
        return Ok(());
    }
    if polynomial.is_zero() {
        return Err(GenericTensorPolynomialError::ZeroWeightDenominator { source_term });
    }
    let origin = TensorPolynomialWeightGuardOrigin { source_term };
    if let Some(condition) = conditions
        .iter_mut()
        .find(|condition| condition.polynomial == polynomial)
    {
        if condition.origins.insert(origin) {
            stats.weight_guard_origins = checked_add(
                stats.weight_guard_origins,
                1,
                "tensor polynomial weight guard origins",
            )?;
            check_limit(
                "tensor polynomial weight guard origins",
                stats.weight_guard_origins,
                limits.max_weight_guard_origins,
            )?;
        }
        return Ok(());
    }
    check_limit(
        "tensor polynomial weight nonzero conditions",
        checked_add(
            conditions.len(),
            1,
            "tensor polynomial weight nonzero conditions",
        )?,
        limits.max_weight_nonzero_conditions,
    )?;
    let polynomial_terms = polynomial.coefficients.len();
    let polynomial_exponents = polynomial.exponents.len();
    let prior_terms = conditions.iter().try_fold(0_usize, |total, condition| {
        checked_add(
            total,
            condition.polynomial.coefficients.len(),
            "tensor polynomial weight guard polynomial terms",
        )
    })?;
    let prior_exponents = conditions.iter().try_fold(0_usize, |total, condition| {
        checked_add(
            total,
            condition.polynomial.exponents.len(),
            "tensor polynomial weight guard exponent entries",
        )
    })?;
    check_limit(
        "tensor polynomial weight guard polynomial terms",
        checked_add(
            prior_terms,
            polynomial_terms,
            "tensor polynomial weight guard polynomial terms",
        )?,
        limits.max_weight_guard_polynomial_terms,
    )?;
    check_limit(
        "tensor polynomial weight guard exponent entries",
        checked_add(
            prior_exponents,
            polynomial_exponents,
            "tensor polynomial weight guard exponent entries",
        )?,
        limits.max_weight_guard_exponent_entries,
    )?;
    charge_display(
        &polynomial,
        &mut stats.retained_guard_bytes,
        limits.max_retained_guard_bytes,
        "tensor polynomial retained guard bytes",
    )?;
    stats.weight_guard_origins = checked_add(
        stats.weight_guard_origins,
        1,
        "tensor polynomial weight guard origins",
    )?;
    check_limit(
        "tensor polynomial weight guard origins",
        stats.weight_guard_origins,
        limits.max_weight_guard_origins,
    )?;
    conditions.push(TensorPolynomialWeightNonZeroCondition {
        polynomial,
        origins: BTreeSet::from([origin]),
    });
    Ok(())
}

fn preflight_family_copies(
    family: &IntegralFamily,
    copies: usize,
    limits: GenericTensorPolynomialLimits,
) -> Result<(), GenericTensorPolynomialError> {
    check_limit(
        "tensor polynomial family-domain copies",
        copies,
        limits.max_family_domain_copies,
    )?;
    let condition_count = family
        .domain()
        .input_denominators()
        .len()
        .checked_add(1)
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow {
            resource: "tensor polynomial family-domain conditions",
        })?;
    let (terms, exponents) = family
        .domain()
        .input_denominators()
        .iter()
        .chain(std::iter::once(family.domain().determinant_nonzero()))
        .try_fold((0_usize, 0_usize), |(terms, exponents), condition| {
            Ok::<_, GenericTensorPolynomialError>((
                checked_add(
                    terms,
                    condition.polynomial().coefficients.len(),
                    "tensor polynomial family-domain polynomial terms",
                )?,
                checked_add(
                    exponents,
                    condition.polynomial().exponents.len(),
                    "tensor polynomial family-domain exponent entries",
                )?,
            ))
        })?;
    let origins = family
        .domain()
        .input_denominators()
        .iter()
        .chain(std::iter::once(family.domain().determinant_nonzero()))
        .try_fold(0_usize, |total, condition| {
            checked_add(
                total,
                condition.origins().len(),
                "tensor polynomial family-domain origins",
            )
        })?;
    check_product_limit(
        condition_count,
        copies,
        "tensor polynomial family-domain conditions",
        limits.max_family_domain_conditions,
    )?;
    check_product_limit(
        origins,
        copies,
        "tensor polynomial family-domain origins",
        limits.max_family_domain_origins,
    )?;
    check_product_limit(
        terms,
        copies,
        "tensor polynomial family-domain polynomial terms",
        limits.max_family_domain_polynomial_terms,
    )?;
    check_product_limit(
        exponents,
        copies,
        "tensor polynomial family-domain exponent entries",
        limits.max_family_domain_exponent_entries,
    )?;
    let manifest =
        family
            .loop_momenta()
            .iter()
            .try_fold(family.fingerprint().len(), |bytes, label| {
                bytes.checked_add(label.len()).ok_or(
                    GenericTensorPolynomialError::ResourceCountOverflow {
                        resource: "tensor polynomial family manifest bytes",
                    },
                )
            })?;
    check_product_limit(
        manifest,
        copies,
        "tensor polynomial family manifest bytes",
        limits.max_family_manifest_bytes,
    )
}

fn source_entries(source: &CovariantTensorMonomial) -> Result<usize, GenericTensorPolynomialError> {
    source
        .loop_vectors()
        .len()
        .checked_add(source.spectator_vectors().len())
        .and_then(|value| value.checked_add(source.metrics().len()))
        .and_then(|value| value.checked_add(source.loop_scalar_products().factors().len()))
        .and_then(|value| value.checked_add(source.spectator_scalar_products().factors().len()))
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow {
            resource: "tensor polynomial source structure entries",
        })
}

fn polynomial_monomial_entries(
    monomial: &CovariantTensorPolynomialMonomial,
) -> Result<usize, GenericTensorPolynomialError> {
    monomial
        .covariant
        .metrics()
        .metrics()
        .len()
        .checked_add(monomial.covariant.spectator_vectors().len())
        .and_then(|value| {
            value.checked_add(
                monomial
                    .covariant
                    .spectator_scalar_products()
                    .factors()
                    .len(),
            )
        })
        .and_then(|value| value.checked_add(monomial.loop_scalar_products.factors().len()))
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow {
            resource: "tensor polynomial retained structure entries",
        })
}

fn charge_structure(
    value: &impl fmt::Debug,
    entries: usize,
    copies: usize,
    retained_entries: &mut usize,
    retained_bytes: &mut usize,
    limits: GenericTensorPolynomialLimits,
) -> Result<(), GenericTensorPolynomialError> {
    let entry_charge =
        entries
            .checked_mul(copies)
            .ok_or(GenericTensorPolynomialError::ResourceCountOverflow {
                resource: "tensor polynomial retained structure entries",
            })?;
    let requested = checked_add(
        *retained_entries,
        entry_charge,
        "tensor polynomial retained structure entries",
    )?;
    check_limit(
        "tensor polynomial retained structure entries",
        requested,
        limits.max_retained_structure_entries,
    )?;
    for _ in 0..copies {
        charge_debug(
            value,
            retained_bytes,
            limits.max_retained_structure_bytes,
            "tensor polynomial retained structure bytes",
        )?;
    }
    *retained_entries = requested;
    Ok(())
}

fn charge_coefficient(
    coefficient: &Coefficient,
    retained: &mut usize,
    limit: usize,
) -> Result<(), GenericTensorPolynomialError> {
    charge_display(
        coefficient,
        retained,
        limit,
        "tensor polynomial retained coefficient bytes",
    )
}

fn charge_display(
    value: &impl fmt::Display,
    retained: &mut usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), GenericTensorPolynomialError> {
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: limit.saturating_sub(*retained),
    };
    write!(&mut writer, "{value}").map_err(|_| GenericTensorPolynomialError::ResourceLimit {
        resource,
        requested: limit.saturating_add(1),
        limit,
    })?;
    *retained = checked_add(*retained, writer.length, resource)?;
    Ok(())
}

fn charge_debug(
    value: &impl fmt::Debug,
    retained: &mut usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), GenericTensorPolynomialError> {
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: limit.saturating_sub(*retained),
    };
    write!(&mut writer, "{value:?}").map_err(|_| GenericTensorPolynomialError::ResourceLimit {
        resource,
        requested: limit.saturating_add(1),
        limit,
    })?;
    *retained = checked_add(*retained, writer.length, resource)?;
    Ok(())
}

fn record_collection_operation(
    stats: &mut GenericTensorPolynomialStats,
    limits: GenericTensorPolynomialLimits,
) -> Result<(), GenericTensorPolynomialError> {
    stats.collection_operations = checked_add_u64(
        stats.collection_operations,
        1,
        "tensor polynomial collection operations",
    )?;
    check_limit_u64(
        "tensor polynomial collection operations",
        stats.collection_operations,
        limits.max_collection_operations,
    )
}

fn check_product_limit(
    left: usize,
    right: usize,
    resource: &'static str,
    limit: usize,
) -> Result<(), GenericTensorPolynomialError> {
    let requested = left
        .checked_mul(right)
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow { resource })?;
    check_limit(resource, requested, limit)
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, GenericTensorPolynomialError> {
    left.checked_add(right)
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow { resource })
}

fn checked_add_u64(
    left: u64,
    right: u64,
    resource: &'static str,
) -> Result<u64, GenericTensorPolynomialError> {
    left.checked_add(right)
        .ok_or(GenericTensorPolynomialError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GenericTensorPolynomialError> {
    if requested > limit {
        Err(GenericTensorPolynomialError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn check_limit_u64(
    resource: &'static str,
    requested: u64,
    limit: u64,
) -> Result<(), GenericTensorPolynomialError> {
    if requested > limit {
        Err(GenericTensorPolynomialError::WorkLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GenericTensorPolynomialError {
    Projector(GenericTensorProjectorError),
    Certificate(TensorReductionCertificateError),
    ExactAlgebra(ExactAlgebraError),
    WrongFamilyFingerprint {
        expected: Arc<str>,
        actual: Arc<str>,
    },
    ZeroWeightDenominator {
        source_term: usize,
    },
    ProjectionReplayMismatch,
    LoweringReplayMismatch,
    ParametricReplayMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    WorkLimit {
        resource: &'static str,
        requested: u64,
        limit: u64,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for GenericTensorPolynomialError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projector(error) => error.fmt(formatter),
            Self::Certificate(error) => error.fmt(formatter),
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::WrongFamilyFingerprint { expected, actual } => write!(
                formatter,
                "tensor polynomial belongs to family fingerprint {expected:?}, not {actual:?}"
            ),
            Self::ZeroWeightDenominator { source_term } => write!(
                formatter,
                "tensor polynomial source {source_term} has a zero coefficient denominator"
            ),
            Self::ProjectionReplayMismatch => formatter
                .write_str("replayed tensor-polynomial projection differs from its certificate"),
            Self::LoweringReplayMismatch => formatter
                .write_str("replayed tensor-polynomial family lowering differs from certificate"),
            Self::ParametricReplayMismatch => formatter.write_str(
                "replayed tensor-polynomial parametric collection differs from certificate",
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding configured limit {limit}"
            ),
            Self::WorkLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} operations, exceeding configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed its representation")
            }
            Self::InternalVerificationFailure { detail } => {
                write!(formatter, "tensor-polynomial invariant failed: {detail}")
            }
        }
    }
}

impl Error for GenericTensorPolynomialError {}

impl From<GenericTensorProjectorError> for GenericTensorPolynomialError {
    fn from(value: GenericTensorProjectorError) -> Self {
        Self::Projector(value)
    }
}

impl From<TensorReductionCertificateError> for GenericTensorPolynomialError {
    fn from(value: TensorReductionCertificateError) -> Self {
        Self::Certificate(value)
    }
}

impl From<ExactAlgebraError> for GenericTensorPolynomialError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

#[derive(Debug)]
pub enum TensorPolynomialReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    Polynomial(GenericTensorPolynomialError),
    Engine(TensorReductionEngineError<ProviderError>),
}

impl<ProviderError> fmt::Display for TensorPolynomialReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Polynomial(error) => error.fmt(formatter),
            Self::Engine(error) => error.fmt(formatter),
        }
    }
}

impl<ProviderError> Error for TensorPolynomialReductionEngineError<ProviderError> where
    ProviderError: Error + Send + Sync + 'static
{
}

impl<ProviderError> From<GenericTensorPolynomialError>
    for TensorPolynomialReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: GenericTensorPolynomialError) -> Self {
        Self::Polynomial(value)
    }
}

impl<ProviderError> From<TensorReductionEngineError<ProviderError>>
    for TensorPolynomialReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: TensorReductionEngineError<ProviderError>) -> Self {
        Self::Engine(value)
    }
}
