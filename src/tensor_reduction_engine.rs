//! Proof-preserving composition of generic tensor lowering with parametric
//! scalar reduction.
//!
//! The tensor-family bridge produces exact scalar integrals grouped by their
//! remaining free-index metric structures. This module sends every distinct
//! scalar integral through a supplied
//! [`ParametricReductionEngine`](crate::ParametricReductionEngine), multiplies
//! and collects the resulting leaves with checked Symbolica arithmetic, and
//! retains both proof domains and all source provenance. It contains no
//! loop-count, topology, or recurrence special cases.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};
use std::sync::Arc;

use crate::{
    AuthenticatedVacuumCovariantTensorProjection, AuthenticatedVacuumTensorLowering,
    AuthenticatedVacuumTensorProjection, CertifiedRewriteDomainCondition, Coefficient,
    CoefficientContext, ConcreteIntegralKey, ConcreteRuleApplicationTrace, ConcreteRuleProvider,
    ConcreteTerminalStatus, ExactAlgebraError, ExactAlgebraLimits, GenericTensorFamilyError,
    GenericTensorFamilyLimits, GenericTensorFamilyReducer, GenericTensorIntegralReduction,
    GenericTensorNumerator, GenericTensorProjectionDomain, GenericTensorProjectorError,
    GenericTensorTerm, IntegralFamily, IntegralOrderingPolicy, MetricPairing, ParametricIbpError,
    ParametricIbpGenerator, ParametricReductionEngine, ParametricReductionResult,
    ReductionEngineError, SpecializedNonZeroCondition, SpectatorScalarProductMonomial,
    TensorCovariantStructure, TensorLoweringDomain, TensorLoweringOrigin,
};

pub const TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA: &str =
    "rustred-tensor-parametric-reduction-engine-v1";

pub const COVARIANT_TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA: &str =
    "rustred-covariant-tensor-parametric-reduction-engine-v1";

/// Stable semantic version of authenticated projection, family lowering, and
/// parametric scalar reduction as one replayable certificate.
pub const AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-tensor-parametric-reduction-v1";
pub const AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-tensor-parametric-reduction-v2";

/// Stable semantic version of spectator-covariant projection plus generic
/// scalar-product-to-family lowering.
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-lowering-v1";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-lowering-v2";

/// Stable semantic version of the spectator-covariant end-to-end reduction.
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-parametric-reduction-v1";
pub const AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA: &str =
    "rustred-authenticated-vacuum-covariant-tensor-parametric-reduction-v2";

/// Aggregate limits for one tensor/scalar composition certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TensorReductionEngineLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_input_structures: usize,
    pub max_input_scalar_terms: usize,
    pub max_unique_scalar_reductions: usize,
    pub max_scalar_witness_terms: usize,
    pub max_scalar_witness_guards: usize,
    pub max_scalar_witness_guard_origins: usize,
    pub max_scalar_witness_certified_domain_conditions: usize,
    pub max_scalar_witness_certified_domain_origins: usize,
    pub max_scalar_witness_application_traces: usize,
    pub max_scalar_witness_terminal_statuses: usize,
    pub max_output_structures: usize,
    pub max_output_terms: usize,
    pub max_terms_per_structure: usize,
    pub max_output_exponent_entries: usize,
    pub max_sparse_operations: usize,
    pub max_term_origins: usize,
    pub max_output_provenance_factor_entries: usize,
    pub max_composite_guards: usize,
    pub max_guard_sources: usize,
    pub max_guard_source_metric_entries: usize,
    /// Aggregate entries in complete covariant structures cloned into output,
    /// status, terminal-classification, and guard-source records.
    pub max_retained_covariant_structure_entries: usize,
    /// Aggregate bounded Debug bytes for those retained covariant clones.
    pub max_retained_covariant_structure_bytes: usize,
    pub max_retained_coefficient_bytes: usize,
    /// Bounded formatted payload of fingerprints, guard polynomials, and
    /// variable-size guard-origin labels. Structural allocations use the
    /// separate count/entry limits above.
    pub max_retained_guard_and_certificate_text_bytes: usize,
}

impl Default for TensorReductionEngineLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_input_structures: 1_000_000,
            max_input_scalar_terms: 10_000_000,
            max_unique_scalar_reductions: 10_000_000,
            max_scalar_witness_terms: 100_000_000,
            max_scalar_witness_guards: 10_000_000,
            max_scalar_witness_guard_origins: 100_000_000,
            max_scalar_witness_certified_domain_conditions: 10_000_000,
            max_scalar_witness_certified_domain_origins: 100_000_000,
            max_scalar_witness_application_traces: 100_000_000,
            max_scalar_witness_terminal_statuses: 100_000_000,
            max_output_structures: 1_000_000,
            max_output_terms: 100_000_000,
            max_terms_per_structure: 10_000_000,
            max_output_exponent_entries: 1_000_000_000,
            max_sparse_operations: 1_000_000_000,
            max_term_origins: 100_000_000,
            max_output_provenance_factor_entries: 1_000_000_000,
            max_composite_guards: 10_000_000,
            max_guard_sources: 100_000_000,
            max_guard_source_metric_entries: 1_000_000_000,
            max_retained_covariant_structure_entries: 1_000_000_000,
            max_retained_covariant_structure_bytes: 512 * 1024 * 1024,
            max_retained_coefficient_bytes: 1024 * 1024 * 1024,
            max_retained_guard_and_certificate_text_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Checked family lowering of an authenticated spectator-covariant vacuum
/// projection.
///
/// Every complete [`TensorCovariantStructure`] owns an independent generic
/// family lowering whose temporary metric key is empty.  The outer covariant
/// key is authoritative and retained alongside the original projector proof;
/// this avoids encoding spectator vectors as fake family external momenta or
/// losing them in the metric-only bridge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorLowering {
    schema: &'static str,
    projection: AuthenticatedVacuumCovariantTensorProjection,
    base_integral: ConcreteIntegralKey,
    lowering_limits: GenericTensorFamilyLimits,
    stats: CovariantTensorLoweringStats,
    lowerings: BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
}

/// Aggregate work and retained-data accounting across every covariant sector
/// in one family-lowering certificate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CovariantTensorLoweringStats {
    pub input_terms: usize,
    pub source_structure_entries: usize,
    pub covariant_structures: usize,
    pub covariant_structure_entries: usize,
    pub covariant_structure_bytes: usize,
    pub family_domain_copies: usize,
    pub family_domain_conditions: usize,
    pub family_domain_origins: usize,
    pub family_domain_polynomial_terms: usize,
    pub family_domain_exponent_entries: usize,
    pub family_manifest_bytes: usize,
    pub expansion_operations: u64,
    pub output_terms: usize,
    pub output_exponent_entries: usize,
    pub retained_origins: usize,
    pub nonzero_conditions: usize,
    pub nonzero_condition_origins: usize,
    pub retained_coefficient_bytes: usize,
}

impl AuthenticatedVacuumCovariantTensorLowering {
    pub fn try_new(
        projection: AuthenticatedVacuumCovariantTensorProjection,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
    ) -> Result<Self, TensorReductionCertificateError> {
        Self::try_new_with_limits(
            projection,
            family,
            base_integral,
            GenericTensorFamilyLimits::default(),
        )
    }

    pub fn try_new_with_limits(
        projection: AuthenticatedVacuumCovariantTensorProjection,
        family: &IntegralFamily,
        base_integral: &ConcreteIntegralKey,
        lowering_limits: GenericTensorFamilyLimits,
    ) -> Result<Self, TensorReductionCertificateError> {
        projection.verify(family)?;
        let (lowerings, stats) =
            build_covariant_lowerings(&projection, family, base_integral, lowering_limits)?;
        Ok(Self {
            schema: AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V2_SCHEMA,
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

    pub const fn projection(&self) -> &AuthenticatedVacuumCovariantTensorProjection {
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

    pub fn lowering_for_covariant(
        &self,
        covariant: &TensorCovariantStructure,
    ) -> Option<&GenericTensorIntegralReduction> {
        self.lowerings.get(covariant)
    }

    pub fn lowering_domains(
        &self,
    ) -> impl Iterator<Item = (&TensorCovariantStructure, &TensorLoweringDomain)> {
        self.lowerings
            .iter()
            .map(|(covariant, lowering)| (covariant, lowering.domain()))
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), TensorReductionCertificateError> {
        self.projection.verify(family)?;
        let (replay, replay_stats) = build_covariant_lowerings(
            &self.projection,
            family,
            &self.base_integral,
            self.lowering_limits,
        )?;
        if replay == self.lowerings && replay_stats == self.stats {
            Ok(())
        } else {
            Err(TensorReductionCertificateError::CovariantLoweringReplayMismatch)
        }
    }
}

fn build_covariant_lowerings(
    projection: &AuthenticatedVacuumCovariantTensorProjection,
    family: &IntegralFamily,
    base_integral: &ConcreteIntegralKey,
    limits: GenericTensorFamilyLimits,
) -> Result<
    (
        BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
        CovariantTensorLoweringStats,
    ),
    TensorReductionCertificateError,
> {
    build_covariant_numerator_lowerings(projection.numerator(), family, base_integral, limits)
}

pub(crate) fn build_covariant_numerator_lowerings(
    numerator: &crate::GenericCovariantTensorNumerator,
    family: &IntegralFamily,
    base_integral: &ConcreteIntegralKey,
    limits: GenericTensorFamilyLimits,
) -> Result<
    (
        BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
        CovariantTensorLoweringStats,
    ),
    TensorReductionCertificateError,
> {
    if base_integral.powers().len() != family.denominator_count() {
        return Err(TensorReductionCertificateError::WrongArity {
            expected: family.denominator_count(),
            actual: base_integral.powers().len(),
        });
    }
    check_limit(
        "covariant tensor input terms",
        numerator.terms().len(),
        limits.max_input_terms,
    )?;

    let mut grouped = BTreeMap::<TensorCovariantStructure, Vec<GenericTensorTerm>>::new();
    let mut source_structure_entries = 0_usize;
    let mut covariant_structure_entries = 0_usize;
    let mut covariant_structure_bytes = 0_usize;
    for term in numerator.terms() {
        let structure_entries = term
            .covariant()
            .metrics()
            .metrics()
            .len()
            .checked_add(term.covariant().spectator_vectors().len())
            .and_then(|entries| {
                entries.checked_add(term.covariant().spectator_scalar_products().factors().len())
            })
            .and_then(|entries| entries.checked_add(term.loop_scalar_products().factors().len()))
            .ok_or(TensorReductionCertificateError::ResourceCountOverflow {
                resource: "covariant tensor source structure entries",
            })?;
        source_structure_entries = checked_add(
            source_structure_entries,
            structure_entries,
            "covariant tensor source structure entries",
        )?;
        check_limit(
            "covariant tensor source structure entries",
            source_structure_entries,
            limits.max_source_structure_entries,
        )?;
        if !grouped.contains_key(term.covariant()) {
            let attempted = checked_add(grouped.len(), 1, "covariant tensor structures")?;
            check_limit(
                "covariant tensor structures",
                attempted,
                limits.max_covariant_structures,
            )?;
            charge_covariant_structure(
                term.covariant(),
                &mut covariant_structure_entries,
                &mut covariant_structure_bytes,
                limits.max_covariant_structure_entries,
                limits.max_covariant_structure_bytes,
            )?;
        }
        grouped
            .entry(term.covariant().clone())
            .or_default()
            .push(GenericTensorTerm::new(
                term.coefficient().clone(),
                MetricPairing::empty(),
                term.loop_scalar_products().clone(),
            ));
    }
    check_limit(
        "covariant tensor structures",
        grouped.len(),
        limits.max_covariant_structures,
    )?;
    let family_retention = preflight_covariant_family_retention(family, grouped.len(), limits)?;

    let mut lowerings = BTreeMap::new();
    let mut aggregate = CovariantTensorLoweringStats {
        input_terms: numerator.terms().len(),
        source_structure_entries,
        covariant_structures: grouped.len(),
        covariant_structure_entries,
        covariant_structure_bytes,
        family_domain_copies: family_retention.copies,
        family_domain_conditions: family_retention.conditions,
        family_domain_origins: family_retention.origins,
        family_domain_polynomial_terms: family_retention.polynomial_terms,
        family_domain_exponent_entries: family_retention.exponent_entries,
        family_manifest_bytes: family_retention.manifest_bytes,
        ..CovariantTensorLoweringStats::default()
    };
    for (covariant, terms) in grouped {
        let child_limits = GenericTensorFamilyLimits {
            max_input_terms: terms.len(),
            // The complete source manifest was preflighted before any
            // covariant-key allocation; this per-child ceiling only bounds
            // the reducer's live view of its already-accounted slice.
            max_source_structure_entries: limits.max_source_structure_entries,
            max_output_terms: limits
                .max_output_terms
                .saturating_sub(aggregate.output_terms),
            max_output_exponent_entries: limits
                .max_output_exponent_entries
                .saturating_sub(aggregate.output_exponent_entries),
            max_expansion_operations: limits
                .max_expansion_operations
                .saturating_sub(aggregate.expansion_operations),
            max_retained_origins: limits
                .max_retained_origins
                .saturating_sub(aggregate.retained_origins),
            max_nonzero_conditions: limits
                .max_nonzero_conditions
                .saturating_sub(aggregate.nonzero_conditions),
            max_nonzero_condition_origins: limits
                .max_nonzero_condition_origins
                .saturating_sub(aggregate.nonzero_condition_origins),
            max_retained_coefficient_bytes: limits
                .max_retained_coefficient_bytes
                .saturating_sub(aggregate.retained_coefficient_bytes),
            ..limits
        };
        let numerator =
            GenericTensorNumerator::try_new_with_limit(terms, child_limits.max_input_terms)?;
        let reducer = GenericTensorFamilyReducer::with_limits(family, child_limits);
        let lowering = reducer.lower(base_integral, &numerator)?;
        if lowering
            .structures()
            .keys()
            .any(|metrics| !metrics.is_empty())
        {
            return Err(
                TensorReductionCertificateError::InternalVerificationFailure {
                    detail: "covariant lowering produced a nonempty temporary metric key"
                        .to_owned(),
                },
            );
        }
        let child = lowering.stats();
        aggregate.expansion_operations = aggregate
            .expansion_operations
            .checked_add(child.expansion_operations)
            .ok_or(TensorReductionCertificateError::ResourceCountOverflow {
                resource: "covariant tensor expansion operations",
            })?;
        aggregate.output_terms = checked_add(
            aggregate.output_terms,
            child.output_terms,
            "covariant tensor lowered output terms",
        )?;
        aggregate.output_exponent_entries = checked_add(
            aggregate.output_exponent_entries,
            child.output_exponent_entries,
            "covariant tensor lowered output exponent entries",
        )?;
        aggregate.retained_origins = checked_add(
            aggregate.retained_origins,
            child.retained_origins,
            "covariant tensor retained origins",
        )?;
        aggregate.nonzero_conditions = checked_add(
            aggregate.nonzero_conditions,
            child.nonzero_conditions,
            "covariant tensor nonzero conditions",
        )?;
        aggregate.nonzero_condition_origins = checked_add(
            aggregate.nonzero_condition_origins,
            child.nonzero_condition_origins,
            "covariant tensor nonzero-condition origins",
        )?;
        aggregate.retained_coefficient_bytes = checked_add(
            aggregate.retained_coefficient_bytes,
            child.retained_coefficient_bytes,
            "covariant tensor retained coefficient bytes",
        )?;
        lowerings.insert(covariant, lowering);
    }
    Ok((lowerings, aggregate))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CovariantFamilyRetentionStats {
    copies: usize,
    conditions: usize,
    origins: usize,
    polynomial_terms: usize,
    exponent_entries: usize,
    manifest_bytes: usize,
}

fn preflight_covariant_family_retention(
    family: &IntegralFamily,
    copies: usize,
    limits: GenericTensorFamilyLimits,
) -> Result<CovariantFamilyRetentionStats, TensorReductionCertificateError> {
    check_limit(
        "covariant tensor family-domain copies",
        copies,
        limits.max_family_domain_copies,
    )?;

    // `FamilyDomain` physically retains every input condition and a dedicated
    // determinant condition, even when both have the same polynomial. Count
    // the stored representation rather than the de-duplicating `conditions()`
    // iterator used for semantic checks.
    let conditions_per_copy = checked_add(
        family.domain().input_denominators().len(),
        1,
        "covariant tensor family-domain conditions",
    )?;
    let origins_per_copy = family
        .domain()
        .input_denominators()
        .iter()
        .chain(std::iter::once(family.domain().determinant_nonzero()))
        .try_fold(0_usize, |total, condition| {
            checked_add(
                total,
                condition.origins().len(),
                "covariant tensor family-domain origins",
            )
        })?;

    // Besides the condition polynomials, each cloned domain owns the numerator
    // and denominator polynomials of its exact basis determinant.
    let mut domain_polynomials = family
        .domain()
        .input_denominators()
        .iter()
        .map(|condition| condition.polynomial())
        .chain(std::iter::once(
            family.domain().determinant_nonzero().polynomial(),
        ))
        .chain([
            &family.domain().basis_determinant().numerator,
            &family.domain().basis_determinant().denominator,
        ]);
    let (polynomial_terms_per_copy, exponent_entries_per_copy) =
        domain_polynomials.try_fold((0_usize, 0_usize), |(terms, exponents), polynomial| {
            Ok::<_, TensorReductionCertificateError>((
                checked_add(
                    terms,
                    polynomial.coefficients.len(),
                    "covariant tensor family-domain polynomial terms",
                )?,
                checked_add(
                    exponents,
                    polynomial.exponents.len(),
                    "covariant tensor family-domain exponent entries",
                )?,
            ))
        })?;

    let fingerprint = family.fingerprint();
    let conditions = checked_product(
        conditions_per_copy,
        copies,
        "covariant tensor family-domain conditions",
    )?;
    let origins = checked_product(
        origins_per_copy,
        copies,
        "covariant tensor family-domain origins",
    )?;
    let polynomial_terms = checked_product(
        polynomial_terms_per_copy,
        copies,
        "covariant tensor family-domain polynomial terms",
    )?;
    let exponent_entries = checked_product(
        exponent_entries_per_copy,
        copies,
        "covariant tensor family-domain exponent entries",
    )?;
    let manifest_bytes = checked_product(
        fingerprint.len(),
        copies,
        "covariant tensor family manifest bytes",
    )?;
    check_limit(
        "covariant tensor family-domain conditions",
        conditions,
        limits.max_family_domain_conditions,
    )?;
    check_limit(
        "covariant tensor family-domain origins",
        origins,
        limits.max_family_domain_origins,
    )?;
    check_limit(
        "covariant tensor family-domain polynomial terms",
        polynomial_terms,
        limits.max_family_domain_polynomial_terms,
    )?;
    check_limit(
        "covariant tensor family-domain exponent entries",
        exponent_entries,
        limits.max_family_domain_exponent_entries,
    )?;
    check_limit(
        "covariant tensor family manifest bytes",
        manifest_bytes,
        limits.max_family_manifest_bytes,
    )?;
    Ok(CovariantFamilyRetentionStats {
        copies,
        conditions,
        origins,
        polynomial_terms,
        exponent_entries,
        manifest_bytes,
    })
}

fn covariant_structure_entries(
    covariant: &TensorCovariantStructure,
) -> Result<usize, TensorReductionCertificateError> {
    covariant
        .metrics()
        .metrics()
        .len()
        .checked_add(covariant.spectator_vectors().len())
        .and_then(|entries| {
            entries.checked_add(covariant.spectator_scalar_products().factors().len())
        })
        .ok_or(TensorReductionCertificateError::ResourceCountOverflow {
            resource: "retained covariant tensor structure entries",
        })
}

fn charge_covariant_structure(
    covariant: &TensorCovariantStructure,
    retained_entries: &mut usize,
    retained_bytes: &mut usize,
    entry_limit: usize,
    byte_limit: usize,
) -> Result<(), TensorReductionCertificateError> {
    let entries = covariant_structure_entries(covariant)?;
    let requested_entries = checked_add(
        *retained_entries,
        entries,
        "retained covariant tensor structure entries",
    )?;
    check_limit(
        "retained covariant tensor structure entries",
        requested_entries,
        entry_limit,
    )?;
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: byte_limit.saturating_sub(*retained_bytes),
    };
    write!(&mut writer, "{covariant:?}").map_err(|_| {
        TensorReductionCertificateError::ResourceLimit {
            resource: "retained covariant tensor structure bytes",
            requested: byte_limit.saturating_add(1),
            limit: byte_limit,
        }
    })?;
    *retained_bytes = checked_add(
        *retained_bytes,
        writer.length,
        "retained covariant tensor structure bytes",
    )?;
    *retained_entries = requested_entries;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TensorReductionEngineStats {
    input_structures: usize,
    input_scalar_terms: usize,
    unique_scalar_reductions: usize,
    scalar_witness_terms: usize,
    scalar_witness_guards: usize,
    scalar_witness_guard_origins: usize,
    scalar_witness_certified_domain_conditions: usize,
    scalar_witness_certified_domain_origins: usize,
    scalar_witness_application_traces: usize,
    scalar_witness_terminal_statuses: usize,
    sparse_multiplications: usize,
    sparse_additions: usize,
    output_structures: usize,
    output_terms: usize,
    composite_guards: usize,
    retained_covariant_structure_entries: usize,
    retained_covariant_structure_bytes: usize,
    retained_coefficient_bytes: usize,
    retained_guard_and_certificate_text_bytes: usize,
}

impl TensorReductionEngineStats {
    pub const fn input_structures(self) -> usize {
        self.input_structures
    }
    pub const fn input_scalar_terms(self) -> usize {
        self.input_scalar_terms
    }
    pub const fn unique_scalar_reductions(self) -> usize {
        self.unique_scalar_reductions
    }
    pub const fn scalar_witness_terms(self) -> usize {
        self.scalar_witness_terms
    }
    pub const fn scalar_witness_guards(self) -> usize {
        self.scalar_witness_guards
    }
    pub const fn scalar_witness_guard_origins(self) -> usize {
        self.scalar_witness_guard_origins
    }
    pub const fn scalar_witness_certified_domain_conditions(self) -> usize {
        self.scalar_witness_certified_domain_conditions
    }
    pub const fn scalar_witness_certified_domain_origins(self) -> usize {
        self.scalar_witness_certified_domain_origins
    }
    pub const fn scalar_witness_application_traces(self) -> usize {
        self.scalar_witness_application_traces
    }
    pub const fn scalar_witness_terminal_statuses(self) -> usize {
        self.scalar_witness_terminal_statuses
    }
    pub const fn sparse_multiplications(self) -> usize {
        self.sparse_multiplications
    }
    pub const fn sparse_additions(self) -> usize {
        self.sparse_additions
    }
    pub const fn output_structures(self) -> usize {
        self.output_structures
    }
    pub const fn output_terms(self) -> usize {
        self.output_terms
    }
    pub const fn composite_guards(self) -> usize {
        self.composite_guards
    }
    pub const fn retained_covariant_structure_entries(self) -> usize {
        self.retained_covariant_structure_entries
    }
    pub const fn retained_covariant_structure_bytes(self) -> usize {
        self.retained_covariant_structure_bytes
    }
    pub const fn retained_coefficient_bytes(self) -> usize {
        self.retained_coefficient_bytes
    }
    pub const fn retained_guard_and_certificate_text_bytes(self) -> usize {
        self.retained_guard_and_certificate_text_bytes
    }
}

fn metric_only_covariant(metrics: &MetricPairing) -> TensorCovariantStructure {
    TensorCovariantStructure::new(
        metrics.clone(),
        Vec::new(),
        SpectatorScalarProductMonomial::one(),
    )
}

/// Covariant-decorated scalar source that required one parametric guard.
///
/// Metric-only projections use a [`TensorCovariantStructure`] with no
/// spectator vectors or spectator scalar products.  Keeping the complete
/// covariant here prevents distinct Vakint-style spectator structures from
/// ever being merged merely because their metric parts agree.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TensorScalarSource {
    covariant: TensorCovariantStructure,
    integral: ConcreteIntegralKey,
}

impl TensorScalarSource {
    pub const fn covariant(&self) -> &TensorCovariantStructure {
        &self.covariant
    }

    /// Convenience accessor retained for metric-only consumers.
    pub const fn metrics(&self) -> &MetricPairing {
        self.covariant.metrics()
    }

    pub const fn integral(&self) -> &ConcreteIntegralKey {
        &self.integral
    }
}

/// One scalar-engine guard together with every tensor source that used it.
///
/// Conditions are merged only when the complete condition, including its
/// original flat guard provenance, is equal. Conditions with the same
/// polynomial but different derivation provenance remain separate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorReductionGuard {
    condition: SpecializedNonZeroCondition,
    sources: BTreeSet<TensorScalarSource>,
}

impl TensorReductionGuard {
    pub const fn condition(&self) -> &SpecializedNonZeroCondition {
        &self.condition
    }

    pub fn sources(&self) -> &BTreeSet<TensorScalarSource> {
        &self.sources
    }
}

/// Flat origin of one collected tensor-reduction coefficient.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TensorReductionTermOrigin {
    scalar_source: ConcreteIntegralKey,
    tensor_origin: TensorLoweringOrigin,
}

impl TensorReductionTermOrigin {
    pub const fn scalar_source(&self) -> &ConcreteIntegralKey {
        &self.scalar_source
    }

    pub const fn tensor_origin(&self) -> &TensorLoweringOrigin {
        &self.tensor_origin
    }
}

/// One collected terminal coefficient with complete tensor-source history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorReducedCoefficient {
    coefficient: Coefficient,
    origins: BTreeSet<TensorReductionTermOrigin>,
}

impl TensorReducedCoefficient {
    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn origins(&self) -> &BTreeSet<TensorReductionTermOrigin> {
        &self.origins
    }
}

/// A terminal integral qualified by its complete remaining Lorentz covariant.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TensorIntegralLeaf {
    covariant: TensorCovariantStructure,
    integral: ConcreteIntegralKey,
}

impl TensorIntegralLeaf {
    pub const fn covariant(&self) -> &TensorCovariantStructure {
        &self.covariant
    }

    /// Convenience accessor retained for metric-only consumers.
    pub const fn metrics(&self) -> &MetricPairing {
        self.covariant.metrics()
    }

    pub const fn integral(&self) -> &ConcreteIntegralKey {
        &self.integral
    }
}

/// Exact scalar-engine output snapshot retained for collection replay.
///
/// In addition to the collected scalar terms, this retains the complete
/// proof-bearing decisions used by the scalar engine.  Local verification
/// authenticates every retained application trace; [`TensorParametricReductionResult::verify_with_engine`]
/// is the stronger check that asks a scalar engine for independently rebuilt
/// snapshots again.
#[derive(Clone, Debug)]
pub struct ScalarReductionWitness {
    family_fingerprint: Arc<str>,
    source: ConcreteIntegralKey,
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: Vec<SpecializedNonZeroCondition>,
    certified_domain: Vec<CertifiedRewriteDomainCondition>,
    application_traces: Vec<ConcreteRuleApplicationTrace>,
    application_trace_manifests: Vec<Arc<str>>,
    terminal_statuses: BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
}

impl PartialEq for ScalarReductionWitness {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.source == other.source
            && self.terms == other.terms
            && self.required_nonzero == other.required_nonzero
            && self.certified_domain == other.certified_domain
            && self.application_trace_manifests == other.application_trace_manifests
            && self.terminal_statuses == other.terminal_statuses
    }
}

impl Eq for ScalarReductionWitness {}

impl ScalarReductionWitness {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn source(&self) -> &ConcreteIntegralKey {
        &self.source
    }

    pub const fn terms(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.terms
    }

    pub fn required_nonzero(&self) -> &[SpecializedNonZeroCondition] {
        &self.required_nonzero
    }

    pub fn certified_domain(&self) -> &[CertifiedRewriteDomainCondition] {
        &self.certified_domain
    }

    pub fn application_traces(&self) -> &[ConcreteRuleApplicationTrace] {
        &self.application_traces
    }

    /// Schema-bound deterministic snapshots used for exact witness comparison
    /// while the retained proof graph itself deliberately remains non-`Eq`.
    pub fn application_trace_manifests(&self) -> &[Arc<str>] {
        &self.application_trace_manifests
    }

    pub const fn terminal_statuses(
        &self,
    ) -> &BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus> {
        &self.terminal_statuses
    }
}

/// Full proof-domain-preserving tensor/scalar composition result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TensorParametricReductionResult {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    lowering: GenericTensorIntegralReduction,
    scalar_witnesses: BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
    structures:
        BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>>,
    guards: Vec<TensorReductionGuard>,
    terminal_statuses:
        BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>>,
    uncovered_leaves: BTreeSet<TensorIntegralLeaf>,
    selected_masters: BTreeSet<TensorIntegralLeaf>,
    certified_masters: BTreeMap<TensorIntegralLeaf, Arc<str>>,
    limits: TensorReductionEngineLimits,
    stats: TensorReductionEngineStats,
}

impl TensorParametricReductionResult {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn lowering(&self) -> &GenericTensorIntegralReduction {
        &self.lowering
    }

    pub const fn domain(&self) -> &TensorLoweringDomain {
        self.lowering.domain()
    }

    pub const fn scalar_witnesses(&self) -> &BTreeMap<ConcreteIntegralKey, ScalarReductionWitness> {
        &self.scalar_witnesses
    }

    pub const fn structures(
        &self,
    ) -> &BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>>
    {
        &self.structures
    }

    pub fn terms_for_covariant(
        &self,
        covariant: &TensorCovariantStructure,
    ) -> Option<&BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>> {
        self.structures.get(covariant)
    }

    /// Metric-only lookup convenience.  A spectator-covariant result must use
    /// [`Self::terms_for_covariant`] so the full structure participates in the
    /// key.
    pub fn terms_for_structure(
        &self,
        metrics: &MetricPairing,
    ) -> Option<&BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>> {
        self.terms_for_covariant(&metric_only_covariant(metrics))
    }

    pub fn term_for_covariant(
        &self,
        covariant: &TensorCovariantStructure,
        integral: &ConcreteIntegralKey,
    ) -> Option<&TensorReducedCoefficient> {
        self.structures.get(covariant)?.get(integral)
    }

    /// Metric-only lookup convenience.  A spectator-covariant result must use
    /// [`Self::term_for_covariant`] so the full structure participates in the
    /// key.
    pub fn term(
        &self,
        metrics: &MetricPairing,
        integral: &ConcreteIntegralKey,
    ) -> Option<&TensorReducedCoefficient> {
        self.term_for_covariant(&metric_only_covariant(metrics), integral)
    }

    pub fn guards(&self) -> &[TensorReductionGuard] {
        &self.guards
    }

    pub const fn terminal_statuses(
        &self,
    ) -> &BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>>
    {
        &self.terminal_statuses
    }

    pub const fn uncovered_leaves(&self) -> &BTreeSet<TensorIntegralLeaf> {
        &self.uncovered_leaves
    }

    pub const fn selected_masters(&self) -> &BTreeSet<TensorIntegralLeaf> {
        &self.selected_masters
    }

    pub const fn certified_masters(&self) -> &BTreeMap<TensorIntegralLeaf, Arc<str>> {
        &self.certified_masters
    }

    pub const fn limits(&self) -> TensorReductionEngineLimits {
        self.limits
    }

    pub const fn stats(&self) -> TensorReductionEngineStats {
        self.stats
    }

    pub fn is_zero(&self) -> bool {
        self.structures.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.structures.is_empty()
    }

    pub fn len(&self) -> usize {
        self.structures.values().map(BTreeMap::len).sum()
    }

    /// Reject every result with an uncovered terminal. Selected and certified
    /// masters remain distinct and are both accepted.
    pub fn require_complete(&self) -> Result<&Self, IncompleteTensorReductionError> {
        if self.uncovered_leaves.is_empty() {
            Ok(self)
        } else {
            Err(IncompleteTensorReductionError {
                uncovered_leaves: self.uncovered_leaves.clone(),
            })
        }
    }

    /// Independently replay collection from the retained lowering and scalar
    /// witnesses. This verifies the composition certificate without querying
    /// a potentially stateful provider again.
    pub fn verify_collected(
        &self,
        family: &IntegralFamily,
    ) -> Result<(), TensorReductionCertificateError> {
        let replay =
            assemble_from_witnesses(family, &self.lowering, &self.scalar_witnesses, self.limits)?;
        if replay == *self {
            Ok(())
        } else {
            Err(TensorReductionCertificateError::ReplayMismatch)
        }
    }

    /// Ask the supplied engine for every distinct scalar request again and
    /// compare the complete result. The engine may itself satisfy requests
    /// from its authenticated cache; callers wanting provider re-execution can
    /// supply a fresh engine.
    pub fn verify_with_engine<Provider>(
        &self,
        family: &IntegralFamily,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<(), TensorReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        let replay = TensorParametricReductionComposer::with_limits(family, self.limits)
            .reduce(&self.lowering, engine)?;
        if replay == *self {
            Ok(())
        } else {
            Err(TensorReductionCertificateError::ReplayMismatch.into())
        }
    }
}

/// Scalar-reduction result for a source already separated by complete
/// spectator-covariant structures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CovariantTensorParametricReductionResult {
    schema: &'static str,
    family_fingerprint: Arc<str>,
    scalar_witnesses: BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
    structures:
        BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>>,
    guards: Vec<TensorReductionGuard>,
    terminal_statuses:
        BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>>,
    uncovered_leaves: BTreeSet<TensorIntegralLeaf>,
    selected_masters: BTreeSet<TensorIntegralLeaf>,
    certified_masters: BTreeMap<TensorIntegralLeaf, Arc<str>>,
    limits: TensorReductionEngineLimits,
    stats: TensorReductionEngineStats,
}

impl CovariantTensorParametricReductionResult {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn scalar_witnesses(&self) -> &BTreeMap<ConcreteIntegralKey, ScalarReductionWitness> {
        &self.scalar_witnesses
    }

    pub const fn structures(
        &self,
    ) -> &BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>>
    {
        &self.structures
    }

    pub fn terms_for_covariant(
        &self,
        covariant: &TensorCovariantStructure,
    ) -> Option<&BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>> {
        self.structures.get(covariant)
    }

    pub fn term(
        &self,
        covariant: &TensorCovariantStructure,
        integral: &ConcreteIntegralKey,
    ) -> Option<&TensorReducedCoefficient> {
        self.structures.get(covariant)?.get(integral)
    }

    pub fn guards(&self) -> &[TensorReductionGuard] {
        &self.guards
    }

    pub const fn terminal_statuses(
        &self,
    ) -> &BTreeMap<TensorCovariantStructure, BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>>
    {
        &self.terminal_statuses
    }

    pub const fn uncovered_leaves(&self) -> &BTreeSet<TensorIntegralLeaf> {
        &self.uncovered_leaves
    }

    pub const fn selected_masters(&self) -> &BTreeSet<TensorIntegralLeaf> {
        &self.selected_masters
    }

    pub const fn certified_masters(&self) -> &BTreeMap<TensorIntegralLeaf, Arc<str>> {
        &self.certified_masters
    }

    pub const fn limits(&self) -> TensorReductionEngineLimits {
        self.limits
    }

    pub const fn stats(&self) -> TensorReductionEngineStats {
        self.stats
    }

    pub fn is_zero(&self) -> bool {
        self.structures.is_empty()
    }

    pub fn len(&self) -> usize {
        self.structures.values().map(BTreeMap::len).sum()
    }

    pub fn require_complete(&self) -> Result<&Self, IncompleteTensorReductionError> {
        if self.uncovered_leaves.is_empty() {
            Ok(self)
        } else {
            Err(IncompleteTensorReductionError {
                uncovered_leaves: self.uncovered_leaves.clone(),
            })
        }
    }
}

/// Borrowed view of every exceptional domain retained by the authenticated
/// end-to-end tensor reduction.
///
/// The projection domain contains Gram/projector conditions, the lowering
/// domain contains the integral-family determinant and projected-coefficient
/// denominators, `scalar_guards` contains all parametric-rule conditions with
/// the scalar sources that required them, and `scalar_certified_domains`
/// exposes the distinct zero-sector/symmetry/elimination conditions.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedVacuumTensorReductionDomains<'certificate> {
    projection: &'certificate GenericTensorProjectionDomain,
    lowering: &'certificate TensorLoweringDomain,
    scalar_guards: &'certificate [TensorReductionGuard],
    scalar_witnesses: &'certificate BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
}

/// Borrowed view of all domains in a spectator-covariant end-to-end result.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticatedVacuumCovariantTensorReductionDomains<'certificate> {
    projection: &'certificate GenericTensorProjectionDomain,
    lowerings: &'certificate BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
    scalar_guards: &'certificate [TensorReductionGuard],
    scalar_witnesses: &'certificate BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
}

impl<'certificate> AuthenticatedVacuumCovariantTensorReductionDomains<'certificate> {
    pub const fn projection(self) -> &'certificate GenericTensorProjectionDomain {
        self.projection
    }

    pub const fn lowerings(
        self,
    ) -> &'certificate BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction> {
        self.lowerings
    }

    pub const fn scalar_guards(self) -> &'certificate [TensorReductionGuard] {
        self.scalar_guards
    }

    pub fn scalar_certified_domains(
        self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'certificate ConcreteIntegralKey,
            &'certificate [CertifiedRewriteDomainCondition],
        ),
    > {
        self.scalar_witnesses
            .iter()
            .map(|(source, witness)| (source, witness.certified_domain()))
    }

    pub fn scalar_certified_domain(
        self,
        source: &ConcreteIntegralKey,
    ) -> Option<&'certificate [CertifiedRewriteDomainCondition]> {
        self.scalar_witnesses
            .get(source)
            .map(ScalarReductionWitness::certified_domain)
    }
}

impl<'certificate> AuthenticatedVacuumTensorReductionDomains<'certificate> {
    pub const fn projection(self) -> &'certificate GenericTensorProjectionDomain {
        self.projection
    }

    pub const fn lowering(self) -> &'certificate TensorLoweringDomain {
        self.lowering
    }

    pub const fn scalar_guards(self) -> &'certificate [TensorReductionGuard] {
        self.scalar_guards
    }

    pub fn scalar_certified_domains(
        self,
    ) -> impl ExactSizeIterator<
        Item = (
            &'certificate ConcreteIntegralKey,
            &'certificate [CertifiedRewriteDomainCondition],
        ),
    > {
        self.scalar_witnesses
            .iter()
            .map(|(source, witness)| (source, witness.certified_domain()))
    }

    pub fn scalar_certified_domain(
        self,
        source: &ConcreteIntegralKey,
    ) -> Option<&'certificate [CertifiedRewriteDomainCondition]> {
        self.scalar_witnesses
            .get(source)
            .map(ScalarReductionWitness::certified_domain)
    }
}

/// Projection-proof-preserving end-to-end vacuum tensor reduction.
///
/// Unlike extracting [`AuthenticatedVacuumTensorLowering::lowering`] and
/// reducing it separately, this object retains the original tensor monomial,
/// the family-bound projector witness, all projector guards, the exact
/// scalar-product lowering, and every scalar reduction snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumTensorParametricReduction {
    schema: &'static str,
    authenticated_lowering: AuthenticatedVacuumTensorLowering,
    scalar_reduction: TensorParametricReductionResult,
}

impl AuthenticatedVacuumTensorParametricReduction {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn authenticated_lowering(&self) -> &AuthenticatedVacuumTensorLowering {
        &self.authenticated_lowering
    }

    pub const fn projection(&self) -> &AuthenticatedVacuumTensorProjection {
        self.authenticated_lowering.projection()
    }

    pub const fn tensor_lowering(&self) -> &GenericTensorIntegralReduction {
        self.authenticated_lowering.lowering()
    }

    pub const fn scalar_reduction(&self) -> &TensorParametricReductionResult {
        &self.scalar_reduction
    }

    pub fn domains(&self) -> AuthenticatedVacuumTensorReductionDomains<'_> {
        AuthenticatedVacuumTensorReductionDomains {
            projection: self.projection().domain(),
            lowering: self.tensor_lowering().domain(),
            scalar_guards: self.scalar_reduction.guards(),
            scalar_witnesses: self.scalar_reduction.scalar_witnesses(),
        }
    }

    pub fn require_complete(&self) -> Result<&Self, IncompleteTensorReductionError> {
        self.scalar_reduction.require_complete()?;
        Ok(self)
    }

    /// Replay both Symbolica tensor stages and the retained scalar collection
    /// snapshots against the supplied family.
    pub fn verify(&self, family: &IntegralFamily) -> Result<(), TensorReductionCertificateError> {
        self.authenticated_lowering.verify(family)?;
        self.verify_lowering_binding()?;
        self.scalar_reduction.verify_collected(family)
    }

    /// Replay the tensor stages and ask the scalar engine for every distinct
    /// scalar reduction again.
    pub fn verify_with_engine<Provider>(
        &self,
        family: &IntegralFamily,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<(), TensorReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        self.authenticated_lowering
            .verify(family)
            .map_err(TensorReductionCertificateError::from)?;
        self.verify_lowering_binding()?;
        self.scalar_reduction.verify_with_engine(family, engine)
    }

    fn verify_lowering_binding(&self) -> Result<(), TensorReductionCertificateError> {
        if self.scalar_reduction.lowering() == self.authenticated_lowering.lowering() {
            Ok(())
        } else {
            Err(TensorReductionCertificateError::AuthenticatedLoweringMismatch)
        }
    }
}

/// End-to-end reduction retaining a spectator-covariant projector witness,
/// its covariant-keyed family lowerings, and one collected scalar certificate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthenticatedVacuumCovariantTensorParametricReduction {
    schema: &'static str,
    authenticated_lowering: AuthenticatedVacuumCovariantTensorLowering,
    scalar_reduction: CovariantTensorParametricReductionResult,
}

impl AuthenticatedVacuumCovariantTensorParametricReduction {
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub const fn authenticated_lowering(&self) -> &AuthenticatedVacuumCovariantTensorLowering {
        &self.authenticated_lowering
    }

    pub const fn projection(&self) -> &AuthenticatedVacuumCovariantTensorProjection {
        self.authenticated_lowering.projection()
    }

    pub const fn scalar_reduction(&self) -> &CovariantTensorParametricReductionResult {
        &self.scalar_reduction
    }

    pub fn domains(&self) -> AuthenticatedVacuumCovariantTensorReductionDomains<'_> {
        AuthenticatedVacuumCovariantTensorReductionDomains {
            projection: self.projection().domain(),
            lowerings: self.authenticated_lowering.lowerings(),
            scalar_guards: self.scalar_reduction.guards(),
            scalar_witnesses: self.scalar_reduction.scalar_witnesses(),
        }
    }

    pub fn require_complete(&self) -> Result<&Self, IncompleteTensorReductionError> {
        self.scalar_reduction.require_complete()?;
        Ok(self)
    }

    pub fn verify(&self, family: &IntegralFamily) -> Result<(), TensorReductionCertificateError> {
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
            Err(TensorReductionCertificateError::ReplayMismatch)
        }
    }

    pub fn verify_with_engine<Provider>(
        &self,
        family: &IntegralFamily,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<(), TensorReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        self.authenticated_lowering.verify(family)?;
        let replay =
            TensorParametricReductionComposer::with_limits(family, self.scalar_reduction.limits())
                .reduce_authenticated_covariant(self.authenticated_lowering.clone(), engine)?;
        if replay == *self {
            Ok(())
        } else {
            Err(TensorReductionCertificateError::ReplayMismatch.into())
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IncompleteTensorReductionError {
    uncovered_leaves: BTreeSet<TensorIntegralLeaf>,
}

impl IncompleteTensorReductionError {
    pub const fn uncovered_leaves(&self) -> &BTreeSet<TensorIntegralLeaf> {
        &self.uncovered_leaves
    }
}

impl fmt::Display for IncompleteTensorReductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "tensor reduction retains {} uncovered terminal(s)",
            self.uncovered_leaves.len()
        )
    }
}

impl Error for IncompleteTensorReductionError {}

/// Stateless generic composition policy bound to one authenticated family.
pub struct TensorParametricReductionComposer<'family> {
    family: &'family IntegralFamily,
    limits: TensorReductionEngineLimits,
}

impl<'family> TensorParametricReductionComposer<'family> {
    pub fn new(family: &'family IntegralFamily) -> Self {
        Self::with_limits(family, TensorReductionEngineLimits::default())
    }

    pub const fn with_limits(
        family: &'family IntegralFamily,
        limits: TensorReductionEngineLimits,
    ) -> Self {
        Self { family, limits }
    }

    pub const fn family(&self) -> &'family IntegralFamily {
        self.family
    }

    pub const fn limits(&self) -> TensorReductionEngineLimits {
        self.limits
    }

    pub fn reduce<Provider>(
        &self,
        lowering: &GenericTensorIntegralReduction,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<TensorParametricReductionResult, TensorReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        validate_lowering(self.family, lowering, self.limits)?;
        self.validate_engine(engine)?;
        let expected_arity = self.family.denominator_count();

        let mut sources = BTreeSet::new();
        for terms in lowering.structures().values() {
            for source in terms.keys() {
                validate_key_arity(source, expected_arity)?;
                if !sources.contains(source) {
                    let attempted = checked_add(sources.len(), 1, "unique scalar reductions")?;
                    check_limit(
                        "unique scalar reductions",
                        attempted,
                        self.limits.max_unique_scalar_reductions,
                    )?;
                    sources.insert(source.clone());
                }
            }
        }

        let witnesses = self.request_witnesses(sources, engine)?;
        assemble_from_witnesses(self.family, lowering, &witnesses, self.limits).map_err(Into::into)
    }

    /// Compose an authenticated vacuum projection/lowering with freshly
    /// requested scalar reductions and retain the complete proof chain.
    ///
    /// The authenticated lowering is consumed so retaining the certificate
    /// does not require an unaccounted duplicate of its projector Gram data.
    pub fn reduce_authenticated<Provider>(
        &self,
        authenticated_lowering: AuthenticatedVacuumTensorLowering,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<
        AuthenticatedVacuumTensorParametricReduction,
        TensorReductionEngineError<Provider::Error>,
    >
    where
        Provider: ConcreteRuleProvider,
    {
        authenticated_lowering
            .verify(self.family)
            .map_err(TensorReductionCertificateError::from)?;
        let scalar_reduction = self.reduce(authenticated_lowering.lowering(), engine)?;
        Ok(AuthenticatedVacuumTensorParametricReduction {
            schema: AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA,
            authenticated_lowering,
            scalar_reduction,
        })
    }

    /// Compose a spectator-covariant lowering with the generic scalar engine
    /// while preserving the complete covariant key at every output leaf and
    /// guard source.
    pub fn reduce_authenticated_covariant<Provider>(
        &self,
        authenticated_lowering: AuthenticatedVacuumCovariantTensorLowering,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<
        AuthenticatedVacuumCovariantTensorParametricReduction,
        TensorReductionEngineError<Provider::Error>,
    >
    where
        Provider: ConcreteRuleProvider,
    {
        authenticated_lowering.verify(self.family)?;
        let scalar_reduction =
            self.reduce_covariant_lowerings(authenticated_lowering.lowerings(), engine)?;
        Ok(AuthenticatedVacuumCovariantTensorParametricReduction {
            schema: AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA,
            authenticated_lowering,
            scalar_reduction,
        })
    }

    /// Crate-internal composition hook. Public callers reach this only
    /// through an authenticated single-monomial or polynomial lowering
    /// wrapper, never through an unauthenticated naked map.
    pub(crate) fn reduce_covariant_lowerings<Provider>(
        &self,
        lowerings: &BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<CovariantTensorParametricReductionResult, TensorReductionEngineError<Provider::Error>>
    where
        Provider: ConcreteRuleProvider,
    {
        self.validate_engine(engine)?;
        let sources =
            collect_covariant_sources(lowerings, self.family.denominator_count(), self.limits)?;
        let witnesses = self.request_witnesses(sources, engine)?;
        assemble_covariant_from_witnesses(self.family, lowerings, &witnesses, self.limits)
            .map_err(Into::into)
    }

    fn validate_engine<Provider>(
        &self,
        engine: &ParametricReductionEngine<'_, Provider>,
    ) -> Result<(), TensorReductionCertificateError>
    where
        Provider: ConcreteRuleProvider,
    {
        let expected_fingerprint: Arc<str> = Arc::from(self.family.fingerprint());
        if engine.family_fingerprint() != expected_fingerprint.as_ref() {
            return Err(TensorReductionCertificateError::WrongEngineFamily {
                expected: expected_fingerprint,
                actual: Arc::from(engine.family_fingerprint()),
            });
        }
        if !self
            .family
            .coefficient_context()
            .has_same_variable_map(engine.coefficient_context())
        {
            return Err(TensorReductionCertificateError::WrongEngineContext);
        }
        let expected_arity = self.family.denominator_count();
        if engine.index_arity() != expected_arity {
            return Err(TensorReductionCertificateError::WrongEngineArity {
                expected: expected_arity,
                actual: engine.index_arity(),
            });
        }
        Ok(())
    }

    fn request_witnesses<Provider>(
        &self,
        sources: BTreeSet<ConcreteIntegralKey>,
        engine: &mut ParametricReductionEngine<'_, Provider>,
    ) -> Result<
        BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
        TensorReductionEngineError<Provider::Error>,
    >
    where
        Provider: ConcreteRuleProvider,
    {
        let expected_fingerprint: Arc<str> = Arc::from(self.family.fingerprint());
        let mut witnesses = BTreeMap::new();
        let mut accounting = WitnessAccounting::empty();
        for source in sources {
            let result = engine.reduce(&source)?;
            let witness = ScalarReductionWitness::try_from_result(
                self.family,
                &expected_fingerprint,
                &source,
                &result,
                self.limits,
                &mut accounting,
            )?;
            witnesses.insert(source, witness);
        }
        Ok(witnesses)
    }
}

impl ScalarReductionWitness {
    #[allow(clippy::too_many_arguments)]
    fn try_from_result(
        family: &IntegralFamily,
        expected_fingerprint: &Arc<str>,
        requested: &ConcreteIntegralKey,
        result: &ParametricReductionResult,
        limits: TensorReductionEngineLimits,
        accounting: &mut WitnessAccounting,
    ) -> Result<Self, TensorReductionCertificateError> {
        if result.family_fingerprint() != expected_fingerprint.as_ref() {
            return Err(TensorReductionCertificateError::WrongScalarFamily {
                source: requested.clone(),
                expected: expected_fingerprint.clone(),
                actual: Arc::from(result.family_fingerprint()),
            });
        }
        if result.source() != requested {
            return Err(TensorReductionCertificateError::WrongScalarSource {
                requested: requested.clone(),
                actual: result.source().clone(),
            });
        }
        validate_snapshot_parts(
            family,
            requested,
            result.family_fingerprint(),
            result.source(),
            result.terms(),
            result.required_nonzero(),
            result.certified_domain(),
            result.application_traces(),
            result.terminal_statuses(),
            limits,
            accounting,
        )?;
        let application_trace_manifests =
            build_trace_manifests(result.application_traces(), limits, accounting)?;
        let witness = Self {
            family_fingerprint: expected_fingerprint.clone(),
            source: requested.clone(),
            terms: result.terms().clone(),
            required_nonzero: result.required_nonzero().to_vec(),
            certified_domain: result.certified_domain().to_vec(),
            application_traces: result.application_traces().to_vec(),
            application_trace_manifests,
            terminal_statuses: result.terminal_statuses().clone(),
        };
        Ok(witness)
    }
}

fn validate_lowering(
    family: &IntegralFamily,
    lowering: &GenericTensorIntegralReduction,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    lowering.verify(family)?;
    check_limit(
        "input tensor structures",
        lowering.structures().len(),
        limits.max_input_structures,
    )?;
    check_limit(
        "input tensor scalar terms",
        lowering.len(),
        limits.max_input_scalar_terms,
    )?;
    if lowering.retained_coefficient_bytes() > limits.max_retained_coefficient_bytes {
        return Err(TensorReductionCertificateError::ResourceLimit {
            resource: "retained tensor coefficient bytes",
            requested: lowering.retained_coefficient_bytes(),
            limit: limits.max_retained_coefficient_bytes,
        });
    }
    for term in lowering.source_numerator().terms() {
        family
            .coefficient_context()
            .validate_with_limits(term.coefficient(), limits.exact_algebra)?;
    }
    for condition in lowering.domain().family().conditions() {
        validate_base_polynomial(
            family.coefficient_context(),
            condition.polynomial(),
            limits.exact_algebra,
        )?;
        if condition.origins().is_empty() {
            return Err(TensorReductionCertificateError::MissingTensorDomainOrigin);
        }
    }
    for condition in lowering.domain().coefficient_nonzero_conditions() {
        validate_base_polynomial(
            family.coefficient_context(),
            condition.polynomial(),
            limits.exact_algebra,
        )?;
        if condition.origins().is_empty() {
            return Err(TensorReductionCertificateError::MissingTensorDomainOrigin);
        }
    }
    for terms in lowering.structures().values() {
        for (source, coefficient) in terms {
            validate_key_arity(source, family.denominator_count())?;
            family
                .coefficient_context()
                .validate_with_limits(coefficient.coefficient(), limits.exact_algebra)?;
            if coefficient.origins().is_empty() {
                return Err(TensorReductionCertificateError::MissingTensorTermOrigin {
                    source: source.clone(),
                });
            }
        }
    }
    Ok(())
}

fn collect_covariant_sources(
    lowerings: &BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
    expected_arity: usize,
    limits: TensorReductionEngineLimits,
) -> Result<BTreeSet<ConcreteIntegralKey>, TensorReductionCertificateError> {
    check_limit(
        "input tensor structures",
        lowerings.len(),
        limits.max_input_structures,
    )?;
    let mut input_terms = 0_usize;
    let mut sources = BTreeSet::new();
    for lowering in lowerings.values() {
        input_terms = checked_add(input_terms, lowering.len(), "input tensor scalar terms")?;
        check_limit(
            "input tensor scalar terms",
            input_terms,
            limits.max_input_scalar_terms,
        )?;
        let Some(terms) = lowering.terms_for_structure(&MetricPairing::empty()) else {
            if lowering.is_zero() {
                continue;
            }
            return Err(
                TensorReductionCertificateError::InternalVerificationFailure {
                    detail: "covariant lowering has no empty temporary metric structure".to_owned(),
                },
            );
        };
        if lowering.structures().len() != 1 {
            return Err(
                TensorReductionCertificateError::InternalVerificationFailure {
                    detail:
                        "covariant lowering retained more than its empty temporary metric structure"
                            .to_owned(),
                },
            );
        }
        for source in terms.keys() {
            validate_key_arity(source, expected_arity)?;
            if sources.insert(source.clone()) {
                check_limit(
                    "unique scalar reductions",
                    sources.len(),
                    limits.max_unique_scalar_reductions,
                )?;
            }
        }
    }
    Ok(sources)
}

fn validate_base_polynomial(
    context: &CoefficientContext,
    polynomial: &crate::generic_family::BasePolynomial,
    limits: ExactAlgebraLimits,
) -> Result<(), TensorReductionCertificateError> {
    let coefficient: Coefficient = polynomial.clone().into();
    context.validate_with_limits(&coefficient, limits)?;
    if polynomial.is_zero() {
        return Err(TensorReductionCertificateError::ZeroTensorDomainCondition);
    }
    Ok(())
}

fn validate_witness(
    family: &IntegralFamily,
    requested: &ConcreteIntegralKey,
    witness: &ScalarReductionWitness,
    limits: TensorReductionEngineLimits,
    accounting: &mut WitnessAccounting,
) -> Result<(), TensorReductionCertificateError> {
    validate_snapshot_parts(
        family,
        requested,
        &witness.family_fingerprint,
        &witness.source,
        &witness.terms,
        &witness.required_nonzero,
        &witness.certified_domain,
        &witness.application_traces,
        &witness.terminal_statuses,
        limits,
        accounting,
    )?;
    let replayed = build_trace_manifests(&witness.application_traces, limits, accounting)?;
    if replayed != witness.application_trace_manifests {
        return Err(
            TensorReductionCertificateError::ScalarApplicationTraceMismatch {
                source: requested.clone(),
            },
        );
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_snapshot_parts(
    family: &IntegralFamily,
    requested: &ConcreteIntegralKey,
    family_fingerprint: &str,
    source: &ConcreteIntegralKey,
    terms: &BTreeMap<ConcreteIntegralKey, Coefficient>,
    required_nonzero: &[SpecializedNonZeroCondition],
    certified_domain: &[CertifiedRewriteDomainCondition],
    application_traces: &[ConcreteRuleApplicationTrace],
    terminal_statuses: &BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    limits: TensorReductionEngineLimits,
    accounting: &mut WitnessAccounting,
) -> Result<(), TensorReductionCertificateError> {
    let expected_fingerprint: Arc<str> = Arc::from(family.fingerprint());
    if family_fingerprint != expected_fingerprint.as_ref() {
        return Err(TensorReductionCertificateError::WrongScalarFamily {
            source: requested.clone(),
            expected: expected_fingerprint,
            actual: Arc::from(family_fingerprint),
        });
    }
    if source != requested {
        return Err(TensorReductionCertificateError::WrongScalarSource {
            requested: requested.clone(),
            actual: source.clone(),
        });
    }
    validate_key_arity(requested, family.denominator_count())?;

    accounting.scalar_witness_terms = checked_add(
        accounting.scalar_witness_terms,
        terms.len(),
        "scalar witness terms",
    )?;
    check_limit(
        "scalar witness terms",
        accounting.scalar_witness_terms,
        limits.max_scalar_witness_terms,
    )?;
    accounting.scalar_witness_guards = checked_add(
        accounting.scalar_witness_guards,
        required_nonzero.len(),
        "scalar witness guards",
    )?;
    check_limit(
        "scalar witness guards",
        accounting.scalar_witness_guards,
        limits.max_scalar_witness_guards,
    )?;
    accounting.scalar_witness_certified_domain_conditions = checked_add(
        accounting.scalar_witness_certified_domain_conditions,
        certified_domain.len(),
        "scalar witness certified-domain conditions",
    )?;
    check_limit(
        "scalar witness certified-domain conditions",
        accounting.scalar_witness_certified_domain_conditions,
        limits.max_scalar_witness_certified_domain_conditions,
    )?;
    accounting.scalar_witness_application_traces = checked_add(
        accounting.scalar_witness_application_traces,
        application_traces.len(),
        "scalar witness application traces",
    )?;
    check_limit(
        "scalar witness application traces",
        accounting.scalar_witness_application_traces,
        limits.max_scalar_witness_application_traces,
    )?;
    accounting.scalar_witness_terminal_statuses = checked_add(
        accounting.scalar_witness_terminal_statuses,
        terminal_statuses.len(),
        "scalar witness terminal statuses",
    )?;
    check_limit(
        "scalar witness terminal statuses",
        accounting.scalar_witness_terminal_statuses,
        limits.max_scalar_witness_terminal_statuses,
    )?;

    if terms.keys().ne(terminal_statuses.keys()) {
        return Err(
            TensorReductionCertificateError::ScalarTerminalCoverageMismatch {
                source: requested.clone(),
            },
        );
    }
    for (leaf, coefficient) in terms {
        validate_key_arity(leaf, family.denominator_count())?;
        family
            .coefficient_context()
            .validate_with_limits(coefficient, limits.exact_algebra)
            .map_err(TensorReductionCertificateError::ExactAlgebra)?;
        accounting.charge_coefficient(coefficient, limits)?;
    }
    for condition in required_nonzero {
        let polynomial: Coefficient = condition.polynomial().raw().clone().into();
        family
            .coefficient_context()
            .validate_with_limits(&polynomial, limits.exact_algebra)
            .map_err(TensorReductionCertificateError::ExactAlgebra)?;
        if condition.polynomial().is_zero() {
            return Err(TensorReductionCertificateError::ZeroScalarGuard {
                source: requested.clone(),
            });
        }
        if condition.origins().is_empty() {
            return Err(TensorReductionCertificateError::MissingScalarGuardOrigin {
                source: requested.clone(),
            });
        }
        accounting.scalar_witness_guard_origins = checked_add(
            accounting.scalar_witness_guard_origins,
            condition.origins().len(),
            "scalar witness guard origins",
        )?;
        check_limit(
            "scalar witness guard origins",
            accounting.scalar_witness_guard_origins,
            limits.max_scalar_witness_guard_origins,
        )?;
        accounting.charge_display(
            condition.polynomial().raw(),
            limits.max_retained_guard_and_certificate_text_bytes,
        )?;
        for origin in condition.origins() {
            accounting
                .charge_debug(origin, limits.max_retained_guard_and_certificate_text_bytes)?;
        }
    }
    for condition in certified_domain {
        let polynomial: Coefficient = condition.polynomial().clone().into();
        family
            .coefficient_context()
            .validate_with_limits(&polynomial, limits.exact_algebra)
            .map_err(TensorReductionCertificateError::ExactAlgebra)?;
        if condition.polynomial().is_zero() {
            return Err(
                TensorReductionCertificateError::ZeroCertifiedDomainCondition {
                    source: requested.clone(),
                },
            );
        }
        if condition.origins().is_empty() {
            return Err(
                TensorReductionCertificateError::MissingCertifiedDomainOrigin {
                    source: requested.clone(),
                },
            );
        }
        accounting.scalar_witness_certified_domain_origins = checked_add(
            accounting.scalar_witness_certified_domain_origins,
            condition.origins().len(),
            "scalar witness certified-domain origins",
        )?;
        check_limit(
            "scalar witness certified-domain origins",
            accounting.scalar_witness_certified_domain_origins,
            limits.max_scalar_witness_certified_domain_origins,
        )?;
        accounting.charge_display(
            condition.polynomial(),
            limits.max_retained_guard_and_certificate_text_bytes,
        )?;
        for origin in condition.origins() {
            accounting
                .charge_debug(origin, limits.max_retained_guard_and_certificate_text_bytes)?;
        }
    }
    if let Some(first) = application_traces.first()
        && application_trace_source(first) != requested
    {
        return Err(
            TensorReductionCertificateError::InvalidScalarApplicationTrace {
                source: requested.clone(),
                trace_source: application_trace_source(first).clone(),
                detail: "the root trace does not match the scalar request",
            },
        );
    }
    for trace in application_traces {
        validate_application_trace(
            family,
            requested,
            required_nonzero,
            certified_domain,
            trace,
            limits,
        )?;
    }
    for status in terminal_statuses.values() {
        if let ConcreteTerminalStatus::CertifiedMaster {
            certificate_fingerprint,
        } = status
        {
            if certificate_fingerprint.is_empty() {
                return Err(TensorReductionCertificateError::EmptyCertificateFingerprint);
            }
            accounting.charge_certificate_bytes(
                certificate_fingerprint.len(),
                limits.max_retained_guard_and_certificate_text_bytes,
            )?;
        }
    }
    Ok(())
}

fn application_trace_source(trace: &ConcreteRuleApplicationTrace) -> &ConcreteIntegralKey {
    match trace {
        ConcreteRuleApplicationTrace::Parametric(rule) => rule.source(),
        ConcreteRuleApplicationTrace::ConditionalParametric(rule) => rule.source(),
        ConcreteRuleApplicationTrace::CertifiedRewrite(rule) => rule.source(),
        ConcreteRuleApplicationTrace::ProvedZero(zero) => zero.source(),
    }
}

fn validate_application_trace(
    family: &IntegralFamily,
    requested: &ConcreteIntegralKey,
    required_nonzero: &[SpecializedNonZeroCondition],
    certified_domain: &[CertifiedRewriteDomainCondition],
    trace: &ConcreteRuleApplicationTrace,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    let (trace_source, family_fingerprint, trace_guards, trace_domain, valid) = match trace {
        ConcreteRuleApplicationTrace::Parametric(rule) => {
            for target in rule.rhs().keys() {
                validate_key_arity(target, family.denominator_count())?;
            }
            let policy = rule.ordering_policy();
            let retained_candidate_replays = rule
                .replay_application(family, rule.parametric_context())
                .is_ok_and(|replayed| replayed);
            let valid = retained_candidate_replays
                && rule.verify_application(
                    family.coefficient_context(),
                    policy,
                    limits.exact_algebra,
                )?;
            (
                rule.source(),
                rule.family_fingerprint(),
                rule.required_nonzero(),
                &[][..],
                valid,
            )
        }
        ConcreteRuleApplicationTrace::ConditionalParametric(rule) => {
            for target in rule.rhs().keys() {
                validate_key_arity(target, family.denominator_count())?;
            }
            let policy = rule.ordering_policy();
            let retained_rule_replays = rule.replay(family, rule.parametric_context()).is_ok();
            let valid = retained_rule_replays
                && rule.verify_application(
                    family.coefficient_context(),
                    policy,
                    limits.exact_algebra,
                )?;
            (
                rule.source(),
                rule.family_fingerprint(),
                rule.required_nonzero(),
                &[][..],
                valid,
            )
        }
        ConcreteRuleApplicationTrace::CertifiedRewrite(rule) => {
            for target in rule.rhs().keys() {
                validate_key_arity(target, family.denominator_count())?;
            }
            // The persisted ordering enum currently has one schema-bound
            // variant.  Every nonempty rewrite additionally authenticates the
            // same policy through its strict-descent witnesses.
            let policy = rule
                .descent_witnesses()
                .values()
                .next()
                .map_or_else(IntegralOrderingPolicy::default, |witness| witness.policy());
            // Generated quotient/elimination proofs retain the exact K(n)
            // namespace that created them.  Pure symmetry proofs predate any
            // parametric specialization and intentionally retain no such
            // context; `CertifiedConcreteRewrite::replay` ignores the
            // supplied context for that proof kind, so give it a freshly
            // family-authenticated default context.
            let fallback_generator = if rule.parametric_context().is_none() {
                Some(ParametricIbpGenerator::try_new(family)?)
            } else {
                None
            };
            let parametric_context = rule.parametric_context().unwrap_or_else(|| {
                fallback_generator
                    .as_ref()
                    .expect("missing contexts construct a fallback generator")
                    .context()
            });
            let retained_rewrite_replays = rule.replay(family, parametric_context, policy).is_ok();
            let valid = retained_rewrite_replays
                && rule.verify_application(
                    family.coefficient_context(),
                    policy,
                    limits.exact_algebra,
                )?;
            (
                rule.source(),
                rule.family_fingerprint(),
                rule.required_nonzero(),
                rule.domain(),
                valid,
            )
        }
        ConcreteRuleApplicationTrace::ProvedZero(zero) => {
            let valid = zero.replay(family).is_ok();
            (
                zero.source(),
                zero.family_fingerprint(),
                &[][..],
                zero.domain(),
                valid,
            )
        }
    };
    validate_key_arity(trace_source, family.denominator_count())?;
    if family_fingerprint != family.fingerprint() {
        return Err(
            TensorReductionCertificateError::InvalidScalarApplicationTrace {
                source: requested.clone(),
                trace_source: trace_source.clone(),
                detail: "application trace belongs to a different integral family",
            },
        );
    }
    if !valid {
        return Err(
            TensorReductionCertificateError::InvalidScalarApplicationTrace {
                source: requested.clone(),
                trace_source: trace_source.clone(),
                detail: "retained application proof does not replay",
            },
        );
    }
    for guard in trace_guards {
        let retained = required_nonzero.iter().any(|condition| {
            condition.polynomial() == guard.polynomial()
                && guard.origins().is_subset(condition.origins())
        });
        if !retained {
            return Err(
                TensorReductionCertificateError::InvalidScalarApplicationTrace {
                    source: requested.clone(),
                    trace_source: trace_source.clone(),
                    detail: "an application guard is absent from the retained scalar domain",
                },
            );
        }
    }
    for condition in trace_domain {
        let retained = certified_domain.iter().any(|candidate| {
            candidate.polynomial() == condition.polynomial()
                && condition.origins().is_subset(candidate.origins())
        });
        if !retained {
            return Err(
                TensorReductionCertificateError::InvalidScalarApplicationTrace {
                    source: requested.clone(),
                    trace_source: trace_source.clone(),
                    detail: "a certified application condition is absent from the retained scalar domain",
                },
            );
        }
    }
    Ok(())
}

fn build_trace_manifests(
    traces: &[ConcreteRuleApplicationTrace],
    limits: TensorReductionEngineLimits,
    accounting: &mut WitnessAccounting,
) -> Result<Vec<Arc<str>>, TensorReductionCertificateError> {
    let mut manifests = Vec::new();
    for trace in traces {
        let remaining = limits
            .max_retained_guard_and_certificate_text_bytes
            .saturating_sub(accounting.retained_certificate_bytes);
        let mut writer = BoundedStringWriter {
            value: String::new(),
            limit: remaining,
        };
        write!(
            &mut writer,
            "rustred-scalar-application-trace-debug-v1|{trace:?}"
        )
        .map_err(|_| TensorReductionCertificateError::ResourceLimit {
            resource: "retained scalar application-trace manifest bytes",
            requested: limits
                .max_retained_guard_and_certificate_text_bytes
                .saturating_add(1),
            limit: limits.max_retained_guard_and_certificate_text_bytes,
        })?;
        accounting.charge_certificate_bytes(
            writer.value.len(),
            limits.max_retained_guard_and_certificate_text_bytes,
        )?;
        manifests.push(Arc::from(writer.value));
    }
    Ok(manifests)
}

fn assemble_from_witnesses(
    family: &IntegralFamily,
    lowering: &GenericTensorIntegralReduction,
    witnesses: &BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
    limits: TensorReductionEngineLimits,
) -> Result<TensorParametricReductionResult, TensorReductionCertificateError> {
    validate_lowering(family, lowering, limits)?;
    check_limit(
        "unique scalar reductions",
        witnesses.len(),
        limits.max_unique_scalar_reductions,
    )?;
    let expected_fingerprint: Arc<str> = Arc::from(family.fingerprint());
    let mut accounting = WitnessAccounting::new(lowering, limits)?;
    for (source, witness) in witnesses {
        validate_witness(family, source, witness, limits, &mut accounting)?;
    }

    let mut structures = BTreeMap::<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>,
    >::new();
    let mut terminal_statuses = BTreeMap::<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    >::new();
    let mut guards = Vec::<TensorReductionGuard>::new();
    let mut output_terms = 0_usize;
    let mut term_origins = 0_usize;
    let mut provenance_factor_entries = 0_usize;
    let mut guard_sources = 0_usize;
    let mut guard_source_metric_entries = 0_usize;
    let mut retained_covariant_structure_entries = 0_usize;
    let mut retained_covariant_structure_bytes = 0_usize;
    let mut sparse_operations = 0_usize;
    let mut sparse_multiplications = 0_usize;
    let mut sparse_additions = 0_usize;

    for (metrics, scalar_terms) in lowering.structures() {
        let covariant = metric_only_covariant(metrics);
        for (source, lowered) in scalar_terms {
            let witness = witnesses.get(source).ok_or_else(|| {
                TensorReductionCertificateError::MissingScalarWitness {
                    source: source.clone(),
                }
            })?;
            let scalar_source = TensorScalarSource {
                covariant: covariant.clone(),
                integral: source.clone(),
            };
            for condition in &witness.required_nonzero {
                insert_composite_guard(
                    &mut guards,
                    condition,
                    &scalar_source,
                    &mut guard_sources,
                    &mut guard_source_metric_entries,
                    &mut retained_covariant_structure_entries,
                    &mut retained_covariant_structure_bytes,
                    limits,
                    &mut accounting,
                )?;
            }
            if lowered.origins().is_empty() {
                return Err(TensorReductionCertificateError::MissingTensorTermOrigin {
                    source: source.clone(),
                });
            }
            for (leaf, scalar_coefficient) in &witness.terms {
                charge_sparse_operation(&mut sparse_operations, limits)?;
                sparse_multiplications =
                    checked_add(sparse_multiplications, 1, "tensor sparse multiplications")?;
                let product = family.coefficient_context().try_mul(
                    lowered.coefficient(),
                    scalar_coefficient,
                    limits.exact_algebra,
                )?;
                preflight_origin_allocation(
                    lowered.origins(),
                    term_origins,
                    provenance_factor_entries,
                    limits,
                )?;
                let mut origins = BTreeSet::new();
                for tensor_origin in lowered.origins() {
                    origins.insert(TensorReductionTermOrigin {
                        scalar_source: source.clone(),
                        tensor_origin: tensor_origin.clone(),
                    });
                }
                let status = witness.terminal_statuses.get(leaf).ok_or_else(|| {
                    TensorReductionCertificateError::ScalarTerminalCoverageMismatch {
                        source: source.clone(),
                    }
                })?;
                let mutation = insert_output_term(
                    family.coefficient_context(),
                    &mut structures,
                    &mut output_terms,
                    &mut term_origins,
                    &mut provenance_factor_entries,
                    covariant.clone(),
                    leaf.clone(),
                    product,
                    origins,
                    limits,
                    &mut accounting,
                    &mut sparse_operations,
                    &mut sparse_additions,
                    &mut retained_covariant_structure_entries,
                    &mut retained_covariant_structure_bytes,
                )?;
                match mutation {
                    OutputTermMutation::ZeroContribution => {}
                    OutputTermMutation::Inserted => {
                        if !terminal_statuses.contains_key(&covariant) {
                            charge_covariant_structure(
                                &covariant,
                                &mut retained_covariant_structure_entries,
                                &mut retained_covariant_structure_bytes,
                                limits.max_retained_covariant_structure_entries,
                                limits.max_retained_covariant_structure_bytes,
                            )?;
                        }
                        let statuses = terminal_statuses.entry(covariant.clone()).or_default();
                        if statuses.insert(leaf.clone(), status.clone()).is_some() {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail: "a newly inserted tensor leaf retained a stale terminal status"
                                        .to_owned(),
                                },
                            );
                        }
                    }
                    OutputTermMutation::Updated => insert_terminal_status(
                        terminal_statuses.entry(covariant.clone()).or_default(),
                        leaf.clone(),
                        status.clone(),
                    )?,
                    OutputTermMutation::Removed => {
                        let Some(statuses) = terminal_statuses.get_mut(&covariant) else {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail: "a cancelled tensor leaf had no terminal-status map"
                                        .to_owned(),
                                },
                            );
                        };
                        if statuses.remove(leaf).is_none() {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail: "a cancelled tensor leaf had no terminal status"
                                        .to_owned(),
                                },
                            );
                        }
                        if statuses.is_empty() {
                            terminal_statuses.remove(&covariant);
                        }
                    }
                }
            }
        }
    }

    structures.retain(|_, terms| !terms.is_empty());
    terminal_statuses.retain(|metrics, statuses| {
        if let Some(terms) = structures.get(metrics) {
            statuses.retain(|leaf, _| terms.contains_key(leaf));
            !statuses.is_empty()
        } else {
            false
        }
    });
    if structures.keys().ne(terminal_statuses.keys())
        || structures.iter().any(|(metrics, terms)| {
            terminal_statuses
                .get(metrics)
                .is_none_or(|statuses| terms.keys().ne(statuses.keys()))
        })
    {
        return Err(TensorReductionCertificateError::CompositeTerminalCoverageMismatch);
    }
    check_limit(
        "output tensor structures",
        structures.len(),
        limits.max_output_structures,
    )?;

    let (uncovered_leaves, selected_masters, certified_masters) = classify_tensor_terminals(
        &terminal_statuses,
        limits,
        &mut retained_covariant_structure_entries,
        &mut retained_covariant_structure_bytes,
    )?;
    let stats = TensorReductionEngineStats {
        input_structures: lowering.structures().len(),
        input_scalar_terms: lowering.len(),
        unique_scalar_reductions: witnesses.len(),
        scalar_witness_terms: accounting.scalar_witness_terms,
        scalar_witness_guards: accounting.scalar_witness_guards,
        scalar_witness_guard_origins: accounting.scalar_witness_guard_origins,
        scalar_witness_certified_domain_conditions: accounting
            .scalar_witness_certified_domain_conditions,
        scalar_witness_certified_domain_origins: accounting.scalar_witness_certified_domain_origins,
        scalar_witness_application_traces: accounting.scalar_witness_application_traces,
        scalar_witness_terminal_statuses: accounting.scalar_witness_terminal_statuses,
        sparse_multiplications,
        sparse_additions,
        output_structures: structures.len(),
        output_terms,
        composite_guards: guards.len(),
        retained_covariant_structure_entries,
        retained_covariant_structure_bytes,
        retained_coefficient_bytes: accounting.retained_coefficient_bytes,
        retained_guard_and_certificate_text_bytes: accounting.retained_certificate_bytes,
    };
    Ok(TensorParametricReductionResult {
        schema: TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA,
        family_fingerprint: expected_fingerprint,
        lowering: lowering.clone(),
        scalar_witnesses: witnesses.clone(),
        structures,
        guards,
        terminal_statuses,
        uncovered_leaves,
        selected_masters,
        certified_masters,
        limits,
        stats,
    })
}

pub(crate) fn assemble_covariant_from_witnesses(
    family: &IntegralFamily,
    lowerings: &BTreeMap<TensorCovariantStructure, GenericTensorIntegralReduction>,
    witnesses: &BTreeMap<ConcreteIntegralKey, ScalarReductionWitness>,
    limits: TensorReductionEngineLimits,
) -> Result<CovariantTensorParametricReductionResult, TensorReductionCertificateError> {
    let sources = collect_covariant_sources(lowerings, family.denominator_count(), limits)?;
    if sources.len() != witnesses.len()
        || sources.iter().any(|source| !witnesses.contains_key(source))
    {
        let source = sources
            .iter()
            .find(|source| !witnesses.contains_key(*source))
            .cloned()
            .or_else(|| {
                witnesses
                    .keys()
                    .find(|source| !sources.contains(*source))
                    .cloned()
            })
            .unwrap_or_else(|| {
                ConcreteIntegralKey::try_new(vec![0; family.denominator_count()])
                    .expect("family denominator arity is valid")
            });
        return Err(TensorReductionCertificateError::MissingScalarWitness { source });
    }

    let expected_fingerprint: Arc<str> = Arc::from(family.fingerprint());
    let mut accounting = WitnessAccounting::empty();
    let mut input_scalar_terms = 0_usize;
    for lowering in lowerings.values() {
        validate_lowering(family, lowering, limits)?;
        accounting.absorb_lowering(lowering, limits)?;
        input_scalar_terms = checked_add(
            input_scalar_terms,
            lowering.len(),
            "input tensor scalar terms",
        )?;
    }
    for (source, witness) in witnesses {
        validate_witness(family, source, witness, limits, &mut accounting)?;
    }

    let mut structures = BTreeMap::<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>,
    >::new();
    let mut terminal_statuses = BTreeMap::<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    >::new();
    let mut guards = Vec::<TensorReductionGuard>::new();
    let mut output_terms = 0_usize;
    let mut term_origins = 0_usize;
    let mut provenance_factor_entries = 0_usize;
    let mut guard_sources = 0_usize;
    let mut guard_source_metric_entries = 0_usize;
    let mut retained_covariant_structure_entries = 0_usize;
    let mut retained_covariant_structure_bytes = 0_usize;
    let mut sparse_operations = 0_usize;
    let mut sparse_multiplications = 0_usize;
    let mut sparse_additions = 0_usize;

    for (covariant, lowering) in lowerings {
        let Some(scalar_terms) = lowering.terms_for_structure(&MetricPairing::empty()) else {
            if lowering.is_zero() {
                continue;
            }
            return Err(
                TensorReductionCertificateError::InternalVerificationFailure {
                    detail: "covariant lowering lost its empty temporary metric key".to_owned(),
                },
            );
        };
        for (source, lowered) in scalar_terms {
            let witness = witnesses.get(source).ok_or_else(|| {
                TensorReductionCertificateError::MissingScalarWitness {
                    source: source.clone(),
                }
            })?;
            let scalar_source = TensorScalarSource {
                covariant: covariant.clone(),
                integral: source.clone(),
            };
            for condition in &witness.required_nonzero {
                insert_composite_guard(
                    &mut guards,
                    condition,
                    &scalar_source,
                    &mut guard_sources,
                    &mut guard_source_metric_entries,
                    &mut retained_covariant_structure_entries,
                    &mut retained_covariant_structure_bytes,
                    limits,
                    &mut accounting,
                )?;
            }
            if lowered.origins().is_empty() {
                return Err(TensorReductionCertificateError::MissingTensorTermOrigin {
                    source: source.clone(),
                });
            }
            for (leaf, scalar_coefficient) in &witness.terms {
                charge_sparse_operation(&mut sparse_operations, limits)?;
                sparse_multiplications =
                    checked_add(sparse_multiplications, 1, "tensor sparse multiplications")?;
                let product = family.coefficient_context().try_mul(
                    lowered.coefficient(),
                    scalar_coefficient,
                    limits.exact_algebra,
                )?;
                preflight_origin_allocation(
                    lowered.origins(),
                    term_origins,
                    provenance_factor_entries,
                    limits,
                )?;
                let origins = lowered
                    .origins()
                    .iter()
                    .cloned()
                    .map(|tensor_origin| TensorReductionTermOrigin {
                        scalar_source: source.clone(),
                        tensor_origin,
                    })
                    .collect();
                let status = witness.terminal_statuses.get(leaf).ok_or_else(|| {
                    TensorReductionCertificateError::ScalarTerminalCoverageMismatch {
                        source: source.clone(),
                    }
                })?;
                let mutation = insert_output_term(
                    family.coefficient_context(),
                    &mut structures,
                    &mut output_terms,
                    &mut term_origins,
                    &mut provenance_factor_entries,
                    covariant.clone(),
                    leaf.clone(),
                    product,
                    origins,
                    limits,
                    &mut accounting,
                    &mut sparse_operations,
                    &mut sparse_additions,
                    &mut retained_covariant_structure_entries,
                    &mut retained_covariant_structure_bytes,
                )?;
                match mutation {
                    OutputTermMutation::ZeroContribution => {}
                    OutputTermMutation::Inserted => {
                        if !terminal_statuses.contains_key(covariant) {
                            charge_covariant_structure(
                                covariant,
                                &mut retained_covariant_structure_entries,
                                &mut retained_covariant_structure_bytes,
                                limits.max_retained_covariant_structure_entries,
                                limits.max_retained_covariant_structure_bytes,
                            )?;
                        }
                        let statuses = terminal_statuses.entry(covariant.clone()).or_default();
                        if statuses.insert(leaf.clone(), status.clone()).is_some() {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail:
                                        "new covariant tensor leaf retained a stale terminal status"
                                            .to_owned(),
                                },
                            );
                        }
                    }
                    OutputTermMutation::Updated => insert_terminal_status(
                        terminal_statuses.entry(covariant.clone()).or_default(),
                        leaf.clone(),
                        status.clone(),
                    )?,
                    OutputTermMutation::Removed => {
                        let Some(statuses) = terminal_statuses.get_mut(covariant) else {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail:
                                        "cancelled covariant tensor leaf had no terminal-status map"
                                            .to_owned(),
                                },
                            );
                        };
                        if statuses.remove(leaf).is_none() {
                            return Err(
                                TensorReductionCertificateError::InternalVerificationFailure {
                                    detail:
                                        "cancelled covariant tensor leaf had no terminal status"
                                            .to_owned(),
                                },
                            );
                        }
                        if statuses.is_empty() {
                            terminal_statuses.remove(covariant);
                        }
                    }
                }
            }
        }
    }

    structures.retain(|_, terms| !terms.is_empty());
    terminal_statuses.retain(|covariant, statuses| {
        if let Some(terms) = structures.get(covariant) {
            statuses.retain(|leaf, _| terms.contains_key(leaf));
            !statuses.is_empty()
        } else {
            false
        }
    });
    if structures.keys().ne(terminal_statuses.keys())
        || structures.iter().any(|(covariant, terms)| {
            terminal_statuses
                .get(covariant)
                .is_none_or(|statuses| terms.keys().ne(statuses.keys()))
        })
    {
        return Err(TensorReductionCertificateError::CompositeTerminalCoverageMismatch);
    }
    let (uncovered_leaves, selected_masters, certified_masters) = classify_tensor_terminals(
        &terminal_statuses,
        limits,
        &mut retained_covariant_structure_entries,
        &mut retained_covariant_structure_bytes,
    )?;
    let stats = TensorReductionEngineStats {
        input_structures: lowerings.len(),
        input_scalar_terms,
        unique_scalar_reductions: witnesses.len(),
        scalar_witness_terms: accounting.scalar_witness_terms,
        scalar_witness_guards: accounting.scalar_witness_guards,
        scalar_witness_guard_origins: accounting.scalar_witness_guard_origins,
        scalar_witness_certified_domain_conditions: accounting
            .scalar_witness_certified_domain_conditions,
        scalar_witness_certified_domain_origins: accounting.scalar_witness_certified_domain_origins,
        scalar_witness_application_traces: accounting.scalar_witness_application_traces,
        scalar_witness_terminal_statuses: accounting.scalar_witness_terminal_statuses,
        sparse_multiplications,
        sparse_additions,
        output_structures: structures.len(),
        output_terms,
        composite_guards: guards.len(),
        retained_covariant_structure_entries,
        retained_covariant_structure_bytes,
        retained_coefficient_bytes: accounting.retained_coefficient_bytes,
        retained_guard_and_certificate_text_bytes: accounting.retained_certificate_bytes,
    };
    Ok(CovariantTensorParametricReductionResult {
        schema: COVARIANT_TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA,
        family_fingerprint: expected_fingerprint,
        scalar_witnesses: witnesses.clone(),
        structures,
        guards,
        terminal_statuses,
        uncovered_leaves,
        selected_masters,
        certified_masters,
        limits,
        stats,
    })
}

fn preflight_origin_allocation(
    origins: &BTreeSet<TensorLoweringOrigin>,
    aggregate_origins: usize,
    aggregate_factor_entries: usize,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    let requested_origins = checked_add(
        aggregate_origins,
        origins.len(),
        "tensor output term origins",
    )?;
    check_limit(
        "tensor output term origins",
        requested_origins,
        limits.max_term_origins,
    )?;
    let factor_entries = origins.iter().try_fold(0_usize, |total, origin| {
        checked_add(
            total,
            origin.scalar_products().factors().len(),
            "tensor output provenance factor entries",
        )
    })?;
    let requested_factors = checked_add(
        aggregate_factor_entries,
        factor_entries,
        "tensor output provenance factor entries",
    )?;
    check_limit(
        "tensor output provenance factor entries",
        requested_factors,
        limits.max_output_provenance_factor_entries,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutputTermMutation {
    ZeroContribution,
    Inserted,
    Updated,
    Removed,
}

#[allow(clippy::too_many_arguments)]
fn insert_output_term(
    context: &CoefficientContext,
    structures: &mut BTreeMap<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, TensorReducedCoefficient>,
    >,
    output_terms: &mut usize,
    term_origins: &mut usize,
    provenance_factor_entries: &mut usize,
    covariant: TensorCovariantStructure,
    leaf: ConcreteIntegralKey,
    coefficient: Coefficient,
    origins: BTreeSet<TensorReductionTermOrigin>,
    limits: TensorReductionEngineLimits,
    accounting: &mut WitnessAccounting,
    sparse_operations: &mut usize,
    sparse_additions: &mut usize,
    retained_covariant_structure_entries: &mut usize,
    retained_covariant_structure_bytes: &mut usize,
) -> Result<OutputTermMutation, TensorReductionCertificateError> {
    if coefficient.is_zero() {
        return Ok(OutputTermMutation::ZeroContribution);
    }
    if !structures.contains_key(&covariant) {
        let attempted = checked_add(structures.len(), 1, "output tensor structures")?;
        check_limit(
            "output tensor structures",
            attempted,
            limits.max_output_structures,
        )?;
        charge_covariant_structure(
            &covariant,
            retained_covariant_structure_entries,
            retained_covariant_structure_bytes,
            limits.max_retained_covariant_structure_entries,
            limits.max_retained_covariant_structure_bytes,
        )?;
    }
    let terms = structures.entry(covariant.clone()).or_default();
    if let Some(existing) = terms.get(&leaf) {
        charge_sparse_operation(sparse_operations, limits)?;
        *sparse_additions = checked_add(*sparse_additions, 1, "tensor sparse additions")?;
        let sum = context.try_add(existing.coefficient(), &coefficient, limits.exact_algebra)?;
        if sum.is_zero() {
            terms.remove(&leaf);
            *output_terms = output_terms.checked_sub(1).ok_or(
                TensorReductionCertificateError::InternalVerificationFailure {
                    detail: "output term counter underflowed".to_owned(),
                },
            )?;
            if terms.is_empty() {
                structures.remove(&covariant);
            }
            return Ok(OutputTermMutation::Removed);
        } else {
            let existing = terms
                .get_mut(&leaf)
                .expect("the nonzero collected tensor term remains present");
            insert_term_origins(
                &mut existing.origins,
                origins,
                term_origins,
                provenance_factor_entries,
                limits,
            )?;
            existing.coefficient = sum;
            accounting.charge_coefficient(&existing.coefficient, limits)?;
            return Ok(OutputTermMutation::Updated);
        }
    }

    let attempted = checked_add(*output_terms, 1, "tensor output terms")?;
    check_limit("tensor output terms", attempted, limits.max_output_terms)?;
    let terms_in_structure = checked_add(terms.len(), 1, "terms per tensor structure")?;
    check_limit(
        "terms per tensor structure",
        terms_in_structure,
        limits.max_terms_per_structure,
    )?;
    let exponent_entries = attempted.checked_mul(leaf.powers().len()).ok_or(
        TensorReductionCertificateError::ResourceCountOverflow {
            resource: "tensor output exponent entries",
        },
    )?;
    check_limit(
        "tensor output exponent entries",
        exponent_entries,
        limits.max_output_exponent_entries,
    )?;
    let mut retained_origins = BTreeSet::new();
    insert_term_origins(
        &mut retained_origins,
        origins,
        term_origins,
        provenance_factor_entries,
        limits,
    )?;
    accounting.charge_coefficient(&coefficient, limits)?;
    terms.insert(
        leaf,
        TensorReducedCoefficient {
            coefficient,
            origins: retained_origins,
        },
    );
    *output_terms = attempted;
    Ok(OutputTermMutation::Inserted)
}

fn insert_term_origins(
    target: &mut BTreeSet<TensorReductionTermOrigin>,
    origins: BTreeSet<TensorReductionTermOrigin>,
    aggregate_origins: &mut usize,
    aggregate_factor_entries: &mut usize,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    for origin in origins {
        if target.contains(&origin) {
            continue;
        }
        let next_origins = checked_add(*aggregate_origins, 1, "tensor output term origins")?;
        check_limit(
            "tensor output term origins",
            next_origins,
            limits.max_term_origins,
        )?;
        let factors = origin.tensor_origin.scalar_products().factors().len();
        let next_factors = checked_add(
            *aggregate_factor_entries,
            factors,
            "tensor output provenance factor entries",
        )?;
        check_limit(
            "tensor output provenance factor entries",
            next_factors,
            limits.max_output_provenance_factor_entries,
        )?;
        target.insert(origin);
        *aggregate_origins = next_origins;
        *aggregate_factor_entries = next_factors;
    }
    Ok(())
}

fn insert_composite_guard(
    guards: &mut Vec<TensorReductionGuard>,
    condition: &SpecializedNonZeroCondition,
    source: &TensorScalarSource,
    guard_sources: &mut usize,
    guard_source_metric_entries: &mut usize,
    retained_covariant_structure_entries: &mut usize,
    retained_covariant_structure_bytes: &mut usize,
    limits: TensorReductionEngineLimits,
    accounting: &mut WitnessAccounting,
) -> Result<(), TensorReductionCertificateError> {
    if let Some(existing) = guards
        .iter_mut()
        .find(|existing| &existing.condition == condition)
    {
        if existing.sources.contains(source) {
            return Ok(());
        }
        charge_guard_source(
            source,
            guard_sources,
            guard_source_metric_entries,
            retained_covariant_structure_entries,
            retained_covariant_structure_bytes,
            limits,
        )?;
        existing.sources.insert(source.clone());
        return Ok(());
    }
    let attempted = checked_add(guards.len(), 1, "tensor composite guards")?;
    check_limit(
        "tensor composite guards",
        attempted,
        limits.max_composite_guards,
    )?;
    charge_guard_source(
        source,
        guard_sources,
        guard_source_metric_entries,
        retained_covariant_structure_entries,
        retained_covariant_structure_bytes,
        limits,
    )?;
    accounting.charge_display(
        condition.polynomial().raw(),
        limits.max_retained_guard_and_certificate_text_bytes,
    )?;
    for origin in condition.origins() {
        accounting.charge_debug(origin, limits.max_retained_guard_and_certificate_text_bytes)?;
    }
    guards.push(TensorReductionGuard {
        condition: condition.clone(),
        sources: BTreeSet::from([source.clone()]),
    });
    Ok(())
}

fn charge_guard_source(
    source: &TensorScalarSource,
    guard_sources: &mut usize,
    guard_source_metric_entries: &mut usize,
    retained_covariant_structure_entries: &mut usize,
    retained_covariant_structure_bytes: &mut usize,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    let next_sources = checked_add(*guard_sources, 1, "tensor guard sources")?;
    check_limit(
        "tensor guard sources",
        next_sources,
        limits.max_guard_sources,
    )?;
    let covariant_entries = source
        .covariant
        .metrics()
        .metrics()
        .len()
        .checked_add(source.covariant.spectator_vectors().len())
        .and_then(|entries| {
            entries.checked_add(source.covariant.spectator_scalar_products().factors().len())
        })
        .ok_or(TensorReductionCertificateError::ResourceCountOverflow {
            resource: "tensor guard source covariant entries",
        })?;
    let next_metrics = checked_add(
        *guard_source_metric_entries,
        covariant_entries,
        "tensor guard source covariant entries",
    )?;
    check_limit(
        "tensor guard source covariant entries",
        next_metrics,
        limits.max_guard_source_metric_entries,
    )?;
    charge_covariant_structure(
        source.covariant(),
        retained_covariant_structure_entries,
        retained_covariant_structure_bytes,
        limits.max_retained_covariant_structure_entries,
        limits.max_retained_covariant_structure_bytes,
    )?;
    *guard_sources = next_sources;
    *guard_source_metric_entries = next_metrics;
    Ok(())
}

fn insert_terminal_status(
    statuses: &mut BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    integral: ConcreteIntegralKey,
    status: ConcreteTerminalStatus,
) -> Result<(), TensorReductionCertificateError> {
    if let Some(existing) = statuses.get(&integral) {
        if existing != &status {
            return Err(TensorReductionCertificateError::ConflictingTerminalStatus {
                integral,
                first: existing.clone(),
                second: status,
            });
        }
    } else {
        statuses.insert(integral, status);
    }
    Ok(())
}

fn classify_tensor_terminals(
    statuses: &BTreeMap<
        TensorCovariantStructure,
        BTreeMap<ConcreteIntegralKey, ConcreteTerminalStatus>,
    >,
    limits: TensorReductionEngineLimits,
    retained_covariant_structure_entries: &mut usize,
    retained_covariant_structure_bytes: &mut usize,
) -> Result<
    (
        BTreeSet<TensorIntegralLeaf>,
        BTreeSet<TensorIntegralLeaf>,
        BTreeMap<TensorIntegralLeaf, Arc<str>>,
    ),
    TensorReductionCertificateError,
> {
    let mut uncovered = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut certified = BTreeMap::new();
    for (covariant, structure_statuses) in statuses {
        for (integral, status) in structure_statuses {
            charge_covariant_structure(
                covariant,
                retained_covariant_structure_entries,
                retained_covariant_structure_bytes,
                limits.max_retained_covariant_structure_entries,
                limits.max_retained_covariant_structure_bytes,
            )?;
            let leaf = TensorIntegralLeaf {
                covariant: covariant.clone(),
                integral: integral.clone(),
            };
            match status {
                ConcreteTerminalStatus::Uncovered => {
                    uncovered.insert(leaf);
                }
                ConcreteTerminalStatus::SelectedMaster => {
                    selected.insert(leaf);
                }
                ConcreteTerminalStatus::CertifiedMaster {
                    certificate_fingerprint,
                } => {
                    certified.insert(leaf, certificate_fingerprint.clone());
                }
            }
        }
    }
    Ok((uncovered, selected, certified))
}

fn validate_key_arity(
    key: &ConcreteIntegralKey,
    expected: usize,
) -> Result<(), TensorReductionCertificateError> {
    if key.powers().len() == expected {
        Ok(())
    } else {
        Err(TensorReductionCertificateError::WrongArity {
            expected,
            actual: key.powers().len(),
        })
    }
}

fn charge_sparse_operation(
    operations: &mut usize,
    limits: TensorReductionEngineLimits,
) -> Result<(), TensorReductionCertificateError> {
    let attempted = checked_add(*operations, 1, "tensor sparse operations")?;
    check_limit(
        "tensor sparse operations",
        attempted,
        limits.max_sparse_operations,
    )?;
    *operations = attempted;
    Ok(())
}

struct WitnessAccounting {
    scalar_witness_terms: usize,
    scalar_witness_guards: usize,
    scalar_witness_guard_origins: usize,
    scalar_witness_certified_domain_conditions: usize,
    scalar_witness_certified_domain_origins: usize,
    scalar_witness_application_traces: usize,
    scalar_witness_terminal_statuses: usize,
    retained_coefficient_bytes: usize,
    retained_certificate_bytes: usize,
}

impl WitnessAccounting {
    const fn empty() -> Self {
        Self {
            scalar_witness_terms: 0,
            scalar_witness_guards: 0,
            scalar_witness_guard_origins: 0,
            scalar_witness_certified_domain_conditions: 0,
            scalar_witness_certified_domain_origins: 0,
            scalar_witness_application_traces: 0,
            scalar_witness_terminal_statuses: 0,
            retained_coefficient_bytes: 0,
            retained_certificate_bytes: 0,
        }
    }

    fn new(
        lowering: &GenericTensorIntegralReduction,
        limits: TensorReductionEngineLimits,
    ) -> Result<Self, TensorReductionCertificateError> {
        check_limit(
            "retained tensor coefficient bytes",
            lowering.retained_coefficient_bytes(),
            limits.max_retained_coefficient_bytes,
        )?;
        let mut accounting = Self::empty();
        accounting.retained_coefficient_bytes = lowering.retained_coefficient_bytes();
        accounting.charge_certificate_bytes(
            lowering.family_fingerprint().len(),
            limits.max_retained_guard_and_certificate_text_bytes,
        )?;
        Ok(accounting)
    }

    fn absorb_lowering(
        &mut self,
        lowering: &GenericTensorIntegralReduction,
        limits: TensorReductionEngineLimits,
    ) -> Result<(), TensorReductionCertificateError> {
        self.retained_coefficient_bytes = checked_add(
            self.retained_coefficient_bytes,
            lowering.retained_coefficient_bytes(),
            "retained tensor coefficient bytes",
        )?;
        check_limit(
            "retained tensor coefficient bytes",
            self.retained_coefficient_bytes,
            limits.max_retained_coefficient_bytes,
        )?;
        self.charge_certificate_bytes(
            lowering.family_fingerprint().len(),
            limits.max_retained_guard_and_certificate_text_bytes,
        )
    }

    fn charge_coefficient(
        &mut self,
        coefficient: &Coefficient,
        limits: TensorReductionEngineLimits,
    ) -> Result<(), TensorReductionCertificateError> {
        let mut writer = BoundedLengthWriter {
            length: 0,
            limit: limits
                .max_retained_coefficient_bytes
                .saturating_sub(self.retained_coefficient_bytes),
        };
        write!(&mut writer, "{coefficient}").map_err(|_| {
            TensorReductionCertificateError::ResourceLimit {
                resource: "retained tensor/scalar coefficient bytes",
                requested: limits.max_retained_coefficient_bytes.saturating_add(1),
                limit: limits.max_retained_coefficient_bytes,
            }
        })?;
        self.retained_coefficient_bytes = checked_add(
            self.retained_coefficient_bytes,
            writer.length,
            "retained tensor/scalar coefficient bytes",
        )?;
        Ok(())
    }

    fn charge_display(
        &mut self,
        value: &impl fmt::Display,
        limit: usize,
    ) -> Result<(), TensorReductionCertificateError> {
        let mut writer = BoundedLengthWriter {
            length: 0,
            limit: limit.saturating_sub(self.retained_certificate_bytes),
        };
        write!(&mut writer, "{value}").map_err(|_| {
            TensorReductionCertificateError::ResourceLimit {
                resource: "retained tensor reduction certificate bytes",
                requested: limit.saturating_add(1),
                limit,
            }
        })?;
        self.charge_certificate_bytes(writer.length, limit)
    }

    fn charge_debug(
        &mut self,
        value: &impl fmt::Debug,
        limit: usize,
    ) -> Result<(), TensorReductionCertificateError> {
        let mut writer = BoundedLengthWriter {
            length: 0,
            limit: limit.saturating_sub(self.retained_certificate_bytes),
        };
        write!(&mut writer, "{value:?}").map_err(|_| {
            TensorReductionCertificateError::ResourceLimit {
                resource: "retained tensor reduction guard/certificate text bytes",
                requested: limit.saturating_add(1),
                limit,
            }
        })?;
        self.charge_certificate_bytes(writer.length, limit)
    }

    fn charge_certificate_bytes(
        &mut self,
        bytes: usize,
        limit: usize,
    ) -> Result<(), TensorReductionCertificateError> {
        let requested = checked_add(
            self.retained_certificate_bytes,
            bytes,
            "retained tensor reduction certificate bytes",
        )?;
        check_limit(
            "retained tensor reduction certificate bytes",
            requested,
            limit,
        )?;
        self.retained_certificate_bytes = requested;
        Ok(())
    }
}

struct BoundedLengthWriter {
    length: usize,
    limit: usize,
}

struct BoundedStringWriter {
    value: String,
    limit: usize,
}

impl Write for BoundedStringWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let requested = self
            .value
            .len()
            .checked_add(value.len())
            .ok_or(fmt::Error)?;
        if requested > self.limit {
            return Err(fmt::Error);
        }
        self.value.push_str(value);
        Ok(())
    }
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

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, TensorReductionCertificateError> {
    left.checked_add(right)
        .ok_or(TensorReductionCertificateError::ResourceCountOverflow { resource })
}

fn checked_product(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, TensorReductionCertificateError> {
    left.checked_mul(right)
        .ok_or(TensorReductionCertificateError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TensorReductionCertificateError> {
    if requested > limit {
        Err(TensorReductionCertificateError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TensorReductionCertificateError {
    Projector(GenericTensorProjectorError),
    Lowering(GenericTensorFamilyError),
    ParametricIbp(ParametricIbpError),
    WrongEngineArity {
        expected: usize,
        actual: usize,
    },
    WrongEngineFamily {
        expected: Arc<str>,
        actual: Arc<str>,
    },
    WrongEngineContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    WrongScalarFamily {
        source: ConcreteIntegralKey,
        expected: Arc<str>,
        actual: Arc<str>,
    },
    WrongScalarSource {
        requested: ConcreteIntegralKey,
        actual: ConcreteIntegralKey,
    },
    MissingScalarWitness {
        source: ConcreteIntegralKey,
    },
    ScalarTerminalCoverageMismatch {
        source: ConcreteIntegralKey,
    },
    CompositeTerminalCoverageMismatch,
    ConflictingTerminalStatus {
        integral: ConcreteIntegralKey,
        first: ConcreteTerminalStatus,
        second: ConcreteTerminalStatus,
    },
    MissingTensorTermOrigin {
        source: ConcreteIntegralKey,
    },
    MissingTensorDomainOrigin,
    ZeroTensorDomainCondition,
    ZeroScalarGuard {
        source: ConcreteIntegralKey,
    },
    MissingScalarGuardOrigin {
        source: ConcreteIntegralKey,
    },
    ZeroCertifiedDomainCondition {
        source: ConcreteIntegralKey,
    },
    MissingCertifiedDomainOrigin {
        source: ConcreteIntegralKey,
    },
    InvalidScalarApplicationTrace {
        source: ConcreteIntegralKey,
        trace_source: ConcreteIntegralKey,
        detail: &'static str,
    },
    ScalarApplicationTraceMismatch {
        source: ConcreteIntegralKey,
    },
    EmptyCertificateFingerprint,
    AuthenticatedLoweringMismatch,
    CovariantLoweringReplayMismatch,
    ReplayMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    ExactAlgebra(ExactAlgebraError),
    InternalVerificationFailure {
        detail: String,
    },
}

impl fmt::Display for TensorReductionCertificateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projector(error) => error.fmt(formatter),
            Self::Lowering(error) => error.fmt(formatter),
            Self::ParametricIbp(error) => error.fmt(formatter),
            Self::WrongEngineArity { expected, actual } => write!(
                formatter,
                "scalar reduction engine has arity {actual}; tensor family needs {expected}"
            ),
            Self::WrongEngineFamily { expected, actual } => write!(
                formatter,
                "scalar reduction engine belongs to family {actual:?}, expected {expected:?}"
            ),
            Self::WrongEngineContext => formatter.write_str(
                "scalar reduction engine uses a different authenticated coefficient context",
            ),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "tensor/scalar integral key has arity {actual}; expected {expected}"
            ),
            Self::WrongScalarFamily {
                source,
                expected,
                actual,
            } => write!(
                formatter,
                "scalar reduction of {source:?} belongs to family {actual:?}, expected {expected:?}"
            ),
            Self::WrongScalarSource { requested, actual } => write!(
                formatter,
                "scalar reduction source {actual:?} does not match request {requested:?}"
            ),
            Self::MissingScalarWitness { source } => {
                write!(
                    formatter,
                    "tensor source {source:?} has no scalar reduction witness"
                )
            }
            Self::ScalarTerminalCoverageMismatch { source } => write!(
                formatter,
                "scalar witness for {source:?} does not classify every surviving leaf exactly once"
            ),
            Self::CompositeTerminalCoverageMismatch => formatter.write_str(
                "collected tensor terminal statuses do not cover every surviving leaf exactly once",
            ),
            Self::ConflictingTerminalStatus {
                integral,
                first,
                second,
            } => write!(
                formatter,
                "tensor leaf {integral:?} has conflicting statuses {first:?} and {second:?}"
            ),
            Self::MissingTensorTermOrigin { source } => write!(
                formatter,
                "lowered tensor source {source:?} has no retained numerator origin"
            ),
            Self::MissingTensorDomainOrigin => {
                formatter.write_str("tensor lowering domain condition has no typed origin")
            }
            Self::ZeroTensorDomainCondition => {
                formatter.write_str("tensor lowering domain requires a zero polynomial")
            }
            Self::ZeroScalarGuard { source } => {
                write!(
                    formatter,
                    "scalar reduction of {source:?} requires a zero guard"
                )
            }
            Self::MissingScalarGuardOrigin { source } => write!(
                formatter,
                "scalar reduction guard for {source:?} has no typed origin"
            ),
            Self::ZeroCertifiedDomainCondition { source } => write!(
                formatter,
                "scalar reduction of {source:?} retains an identically-zero certified-domain condition"
            ),
            Self::MissingCertifiedDomainOrigin { source } => write!(
                formatter,
                "scalar reduction certified-domain condition for {source:?} has no typed origin"
            ),
            Self::InvalidScalarApplicationTrace {
                source,
                trace_source,
                detail,
            } => write!(
                formatter,
                "scalar application trace at {trace_source:?} retained for reduction {source:?} is invalid: {detail}"
            ),
            Self::ScalarApplicationTraceMismatch { source } => write!(
                formatter,
                "scalar application proof trace for {source:?} differs from its retained exact manifest"
            ),
            Self::EmptyCertificateFingerprint => {
                formatter.write_str("certified master has an empty certificate fingerprint")
            }
            Self::AuthenticatedLoweringMismatch => formatter.write_str(
                "parametric tensor result is not bound to the retained authenticated lowering",
            ),
            Self::CovariantLoweringReplayMismatch => formatter.write_str(
                "spectator-covariant family lowering replay differs from the retained certificate",
            ),
            Self::ReplayMismatch => {
                formatter.write_str("tensor/scalar composition replay differs from certificate")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::InternalVerificationFailure { detail } => {
                write!(
                    formatter,
                    "tensor/scalar composition invariant failed: {detail}"
                )
            }
        }
    }
}

impl Error for TensorReductionCertificateError {}

impl From<GenericTensorProjectorError> for TensorReductionCertificateError {
    fn from(value: GenericTensorProjectorError) -> Self {
        Self::Projector(value)
    }
}

impl From<GenericTensorFamilyError> for TensorReductionCertificateError {
    fn from(value: GenericTensorFamilyError) -> Self {
        Self::Lowering(value)
    }
}

impl From<ParametricIbpError> for TensorReductionCertificateError {
    fn from(value: ParametricIbpError) -> Self {
        Self::ParametricIbp(value)
    }
}

impl From<ExactAlgebraError> for TensorReductionCertificateError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

#[derive(Debug)]
pub enum TensorReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    ScalarEngine(ReductionEngineError<ProviderError>),
    Certificate(TensorReductionCertificateError),
}

impl<ProviderError> fmt::Display for TensorReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScalarEngine(error) => error.fmt(formatter),
            Self::Certificate(error) => error.fmt(formatter),
        }
    }
}

impl<ProviderError> Error for TensorReductionEngineError<ProviderError> where
    ProviderError: Error + Send + Sync + 'static
{
}

impl<ProviderError> From<ReductionEngineError<ProviderError>>
    for TensorReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: ReductionEngineError<ProviderError>) -> Self {
        Self::ScalarEngine(value)
    }
}

impl<ProviderError> From<TensorReductionCertificateError>
    for TensorReductionEngineError<ProviderError>
where
    ProviderError: Error + Send + Sync + 'static,
{
    fn from(value: TensorReductionCertificateError) -> Self {
        Self::Certificate(value)
    }
}
