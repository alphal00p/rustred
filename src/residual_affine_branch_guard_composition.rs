//! Complete composition of one residual Boolean branch's nonzero guards.
//!
//! A guarded affine branch owns equalities defining an integer-affine map
//! `n = F(t)` and retains its Coverage V4 nonzero predicates as structural-
//! locus ordinals.  This certificate resolves every such ordinal and composes
//! its exact polynomial `G(n)` through one shared, source-neutral Symbolica
//! plan.  Consequently each entry proves `G(F(t))`; it deliberately does not
//! claim anything about a later translated point `G(F(t) + q)`.
//!
//! Source predicates are never merged, even when their mapped polynomials are
//! equal.  A zero image is a contradiction for the whole branch, but all
//! later source guards are still composed and retained so replay authenticates
//! the complete source manifest rather than a short-circuited prefix.

use std::fmt;
use std::mem::{align_of, size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::prelude::{Integer, PolyVariable};

use crate::parametric_coefficient::{
    ResidualAffineCompositionPlan, ResidualAffineCompositionPlanLogicalMemoryCensus,
    ResidualAffineCompositionPlanStats,
    residual_affine_composition_plan_memory_envelope_from_limits,
};
use crate::residual_affine_branch_system::{
    ResidualAffineBranchSystemFreshGuardAuthorization,
    ResidualAffineBranchSystemLogicalMemoryCensus,
};
use crate::{
    GuardOrigin, IntegralFamily, ParametricCoefficientContext, ParametricCoefficientError,
    ParametricNonZeroCondition, ParametricPolynomial, ResidualAffineBranchSystemCertificate,
    ResidualAffineBranchSystemError, ResidualAffineBranchSystemOutcome,
    ResidualAffineIntegerSystemCertificate, ResidualProductLocusBooleanCoverCertificate,
    ResidualProductLocusBooleanCoverError, ResidualUnitAffineCompositionError,
    ResidualUnitAffineCompositionPlanLimits, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};

/// Stable schema for complete composition of one residual affine branch's
/// original nonzero guards.
pub const RESIDUAL_AFFINE_BRANCH_GUARD_COMPOSITION_V1_SCHEMA: &str =
    "rustred-residual-affine-branch-guard-composition-v1";

/// Explicit nested and aggregate bounds for branch-guard composition.
///
/// The nested limits bound one shared plan and each individual polynomial.
/// The aggregate limits additionally prevent a large guard manifest from
/// resetting the per-polynomial allowance for every source guard.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualAffineBranchGuardCompositionLimits {
    pub composition_plan: ResidualUnitAffineCompositionPlanLimits,
    pub polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub max_family_fingerprint_bytes: usize,
    pub max_context_fingerprint_bytes: usize,
    pub max_scope_fingerprint_comparison_bytes: usize,
    pub max_guards: usize,
    pub max_structural_locus_lookups: usize,
    pub max_total_source_terms: usize,
    pub max_total_source_exponent_entries: usize,
    pub max_total_source_integer_bits: usize,
    pub max_total_expanded_contributions: usize,
    pub max_total_output_term_bound: usize,
    pub max_total_output_terms: usize,
    pub max_total_output_exponent_entry_bound: usize,
    pub max_total_output_exponent_entries: usize,
    pub max_total_power_calls: usize,
    pub max_total_native_power_heap_pairs: usize,
    pub max_total_multiplication_term_pairs: usize,
    pub max_total_addition_term_visits: usize,
    pub max_total_native_integer_bit_work: usize,
    pub max_total_integer_bit_work: usize,
    pub max_retained_entries: usize,
    pub max_retained_polynomial_terms: usize,
    pub max_retained_polynomial_exponent_entries: usize,
    pub max_retained_polynomial_integer_bits: usize,
    pub max_retained_conditions: usize,
    pub max_retained_origins: usize,
    pub max_retained_origin_bytes: usize,
    pub max_payload_comparison_units: usize,
    pub max_payload_comparison_bytes: usize,
    pub max_payload_comparison_integer_bits: usize,
}

impl Default for ResidualAffineBranchGuardCompositionLimits {
    fn default() -> Self {
        Self {
            composition_plan: ResidualUnitAffineCompositionPlanLimits::default(),
            polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_scope_fingerprint_comparison_bytes: 4 * 1024 * 1024,
            max_guards: 64_000_000,
            max_structural_locus_lookups: 64_000_000,
            max_total_source_terms: 512_000_000,
            max_total_source_exponent_entries: 16_000_000_000,
            max_total_source_integer_bits: 4_000_000_000_000_000,
            max_total_expanded_contributions: 512_000_000,
            max_total_output_term_bound: 512_000_000,
            max_total_output_terms: 512_000_000,
            max_total_output_exponent_entry_bound: 32_000_000_000,
            max_total_output_exponent_entries: 16_000_000_000,
            max_total_power_calls: 16_000_000_000,
            max_total_native_power_heap_pairs: 32_000_000_000,
            max_total_multiplication_term_pairs: 32_000_000_000,
            max_total_addition_term_visits: 32_000_000_000,
            max_total_native_integer_bit_work: 16_000_000_000_000_000,
            max_total_integer_bit_work: 16_000_000_000_000_000,
            max_retained_entries: 64_000_000,
            max_retained_polynomial_terms: 1_024_000_000,
            max_retained_polynomial_exponent_entries: 32_000_000_000,
            max_retained_polynomial_integer_bits: 8_000_000_000_000_000,
            max_retained_conditions: 64_000_000,
            max_retained_origins: 64_000_000,
            max_retained_origin_bytes: 64 * 1024 * 1024 * 1024,
            max_payload_comparison_units: 128_000_000_000,
            max_payload_comparison_bytes: 128 * 1024 * 1024 * 1024,
            max_payload_comparison_integer_bits: 16_000_000_000_000_000,
        }
    }
}

/// Exact source, plan, composition, retention, and replay-comparison census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualAffineBranchGuardCompositionStats {
    family_fingerprint_bytes: usize,
    context_fingerprint_bytes: usize,
    scope_fingerprint_comparison_bytes: usize,
    guards: usize,
    structural_locus_lookups: usize,
    total_source_terms: usize,
    total_source_exponent_entries: usize,
    total_source_integer_bits: usize,
    plan_variables: usize,
    plan_full_images: usize,
    plan_geometry_entries_inspected: usize,
    plan_geometry_entries_retained: usize,
    plan_support_entries_retained: usize,
    plan_total_image_terms: usize,
    plan_total_image_exponent_entries: usize,
    plan_largest_image_integer_bits: usize,
    plan_total_image_integer_bits: usize,
    total_expanded_contributions: usize,
    total_output_term_bound: usize,
    total_output_terms: usize,
    total_output_exponent_entry_bound: usize,
    total_output_exponent_entries: usize,
    total_power_calls: usize,
    total_native_power_heap_pairs: usize,
    total_multiplication_term_pairs: usize,
    total_addition_term_visits: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    total_native_integer_bit_work: usize,
    total_integer_bit_work: usize,
    contradictions: usize,
    discharged_nonzero_integer_constants: usize,
    base_assumptions: usize,
    free_index_dependent_conditions: usize,
    retained_entries: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_conditions: usize,
    retained_origins: usize,
    retained_origin_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
}

macro_rules! composition_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ResidualAffineBranchGuardCompositionStats {
    composition_stats_getters!(
        family_fingerprint_bytes,
        context_fingerprint_bytes,
        scope_fingerprint_comparison_bytes,
        guards,
        structural_locus_lookups,
        total_source_terms,
        total_source_exponent_entries,
        total_source_integer_bits,
        plan_variables,
        plan_full_images,
        plan_geometry_entries_inspected,
        plan_geometry_entries_retained,
        plan_support_entries_retained,
        plan_total_image_terms,
        plan_total_image_exponent_entries,
        plan_largest_image_integer_bits,
        plan_total_image_integer_bits,
        total_expanded_contributions,
        total_output_term_bound,
        total_output_terms,
        total_output_exponent_entry_bound,
        total_output_exponent_entries,
        total_power_calls,
        total_native_power_heap_pairs,
        total_multiplication_term_pairs,
        total_addition_term_visits,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bit_bound,
        total_native_integer_bit_work,
        total_integer_bit_work,
        contradictions,
        discharged_nonzero_integer_constants,
        base_assumptions,
        free_index_dependent_conditions,
        retained_entries,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        retained_conditions,
        retained_origins,
        retained_origin_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
    );
}

/// Semantic class of one mapped source guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchGuardCompositionClass {
    /// `G(F(t)) = 0`; the original nonzero guard is impossible.
    Contradiction,
    /// `G(F(t))` is an authenticated nonzero integer constant.
    DischargedNonzeroIntegerConstant,
    /// `G(F(t))` is symbolic only in the base coefficient variables.
    BaseAssumption(ParametricNonZeroCondition),
    /// `G(F(t))` still depends on at least one free index variable.
    FreeIndexDependent(ParametricNonZeroCondition),
}

impl ResidualAffineBranchGuardCompositionClass {
    pub const fn condition(&self) -> Option<&ParametricNonZeroCondition> {
        match self {
            Self::BaseAssumption(condition) | Self::FreeIndexDependent(condition) => {
                Some(condition)
            }
            Self::Contradiction | Self::DischargedNonzeroIntegerConstant => None,
        }
    }
}

/// One source-ordinal-preserving mapped branch guard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualAffineBranchGuardCompositionEntry {
    structural_locus_ordinal: usize,
    mapped_polynomial: ParametricPolynomial,
    class: ResidualAffineBranchGuardCompositionClass,
    polynomial_stats: ResidualUnitAffinePolynomialCompositionStats,
}

impl ResidualAffineBranchGuardCompositionEntry {
    pub const fn structural_locus_ordinal(&self) -> usize {
        self.structural_locus_ordinal
    }

    /// Exact mapped polynomial `G(F(t))`, retained even when its class also
    /// owns a separately authenticated nonzero condition.
    pub const fn mapped_polynomial(&self) -> &ParametricPolynomial {
        &self.mapped_polynomial
    }

    pub const fn class(&self) -> &ResidualAffineBranchGuardCompositionClass {
        &self.class
    }

    pub const fn polynomial_stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.polynomial_stats
    }

    pub const fn composition_stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.polynomial_stats
    }
}

/// Replayable certificate for every original nonzero guard of one residual
/// affine Boolean branch.
#[derive(Clone, Debug)]
pub struct ResidualAffineBranchGuardCompositionCertificate {
    schema: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    source_branch: Arc<ResidualAffineBranchSystemCertificate>,
    entries: Vec<ResidualAffineBranchGuardCompositionEntry>,
    first_contradiction_entry_ordinal: Option<usize>,
    limits: ResidualAffineBranchGuardCompositionLimits,
    stats: ResidualAffineBranchGuardCompositionStats,
}

/// Source-neutral V2 schema. Unlike the frozen public V1 certificate, this
/// bundle can contain only `GeneratedAffineSealedCondition` provenance.
pub(crate) const RESIDUAL_AFFINE_BRANCH_SEALED_GUARD_V2_SCHEMA: &str =
    "rustred-residual-affine-branch-sealed-guard-v2";

#[cfg(test)]
thread_local! {
    static RESIDUAL_AFFINE_SEALED_GUARD_AUTH_CALLS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static RESIDUAL_AFFINE_SEALED_GUARD_LOCAL_COMPARISON_CENSUS_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static RESIDUAL_AFFINE_SEALED_GUARD_MEMORY_CENSUS_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static RESIDUAL_AFFINE_SEALED_GUARD_STRUCTURAL_PLAN_CENSUS_SCANS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_sealed_guard_auth_calls_for_test() {
    RESIDUAL_AFFINE_SEALED_GUARD_AUTH_CALLS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_sealed_guard_auth_calls_for_test() -> usize {
    RESIDUAL_AFFINE_SEALED_GUARD_AUTH_CALLS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_sealed_guard_local_comparison_census_scans_for_test() {
    RESIDUAL_AFFINE_SEALED_GUARD_LOCAL_COMPARISON_CENSUS_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_sealed_guard_local_comparison_census_scans_for_test() -> usize {
    RESIDUAL_AFFINE_SEALED_GUARD_LOCAL_COMPARISON_CENSUS_SCANS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_sealed_guard_memory_census_scans_for_test() {
    RESIDUAL_AFFINE_SEALED_GUARD_MEMORY_CENSUS_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_sealed_guard_memory_census_scans_for_test() -> usize {
    RESIDUAL_AFFINE_SEALED_GUARD_MEMORY_CENSUS_SCANS.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(crate) fn reset_residual_affine_sealed_guard_structural_plan_census_scans_for_test() {
    RESIDUAL_AFFINE_SEALED_GUARD_STRUCTURAL_PLAN_CENSUS_SCANS.with(|calls| calls.set(0));
}

#[cfg(test)]
pub(crate) fn residual_affine_sealed_guard_structural_plan_census_scans_for_test() -> usize {
    RESIDUAL_AFFINE_SEALED_GUARD_STRUCTURAL_PLAN_CENSUS_SCANS.with(std::cell::Cell::get)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSealedGuardLogicalMemoryCensus {
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    plan_retained_owned_logical_bytes: usize,
    plan_compilation_owned_logical_peak_upper_bound: usize,
    entry_prefix_owned_logical_peak_upper_bound: usize,
}

/// Authenticated scalar cost of comparing two equal sealed-guard payloads.
/// Recursive branch and integer-system payloads stop at their owner seams and
/// are charged separately by the enclosing initial-terminal census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSealedGuardPayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

impl ResidualAffineBranchSealedGuardPayloadComparisonCensus {
    pub(crate) const fn units(self) -> usize {
        self.units
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }

    pub(crate) const fn integer_bits(self) -> usize {
        self.integer_bits
    }
}

/// Checked V2 limit-derived pieces for RustRed-owned logical storage.
///
/// Symbolica backend transients are deliberately outside this ownership
/// census. Their term, operation, exponent, and integer-bit work is admitted
/// by the typed polynomial-composition preflight before the selected
/// Symbolica backend call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineBranchSealedGuardMemoryEnvelopeParts {
    guard_retained_owned_logical_bytes_upper_bound: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    plan_retained_owned_logical_bytes_upper_bound: usize,
    plan_compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineBranchSealedGuardMemoryEnvelopeParts {
    pub(crate) const fn guard_retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.guard_retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn plan_retained_owned_logical_bytes_upper_bound(self) -> usize {
        self.plan_retained_owned_logical_bytes_upper_bound
    }

    pub(crate) const fn plan_compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.plan_compilation_owned_logical_peak_upper_bound
    }
}

impl ResidualAffineBranchSealedGuardLogicalMemoryCensus {
    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn plan_retained_owned_logical_bytes(self) -> usize {
        self.plan_retained_owned_logical_bytes
    }

    pub(crate) const fn plan_compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.plan_compilation_owned_logical_peak_upper_bound
    }

    pub(crate) const fn entry_prefix_owned_logical_peak_upper_bound(self) -> usize {
        self.entry_prefix_owned_logical_peak_upper_bound
    }
}

struct ResidualAffineBranchSealedGuardCore {
    schema: &'static str,
    family_fingerprint: String,
    context_fingerprint: String,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    source_branch: Arc<ResidualAffineBranchSystemCertificate>,
    integer_system: Arc<crate::ResidualAffineIntegerSystemCertificate>,
    entries: Vec<ResidualAffineBranchGuardCompositionEntry>,
    first_contradiction_entry_ordinal: Option<usize>,
    limits: ResidualAffineBranchGuardCompositionLimits,
    stats: ResidualAffineBranchGuardCompositionStats,
    memory: ResidualAffineBranchSealedGuardLogicalMemoryCensus,
    payload_comparison_census: PayloadComparisonCensus,
}

/// Opaque, non-`Clone` V2 owner. No method returns a V1 entry, certificate,
/// cover, branch, raw class, owning Arc, or legacy source locator.
pub(crate) struct ResidualAffineBranchSealedGuardBundle {
    core: Arc<ResidualAffineBranchSealedGuardCore>,
}

impl fmt::Debug for ResidualAffineBranchSealedGuardBundle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSealedGuardBundle")
            .field("schema", &self.core.schema)
            .field("guard_count", &self.core.entries.len())
            .field(
                "has_contradiction",
                &self.core.first_contradiction_entry_ordinal.is_some(),
            )
            .field("private_sources", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl ResidualAffineBranchSealedGuardBundle {
    pub(crate) fn guard_count(&self) -> usize {
        self.core.entries.len()
    }

    pub(crate) fn has_contradiction(&self) -> bool {
        self.core.first_contradiction_entry_ordinal.is_some()
    }

    pub(crate) fn first_contradiction_entry_ordinal(&self) -> Option<usize> {
        self.core.first_contradiction_entry_ordinal
    }

    pub(crate) fn memory(&self) -> ResidualAffineBranchSealedGuardLogicalMemoryCensus {
        self.core.memory
    }

    pub(crate) fn compile_fresh_sealed(
        context: &ParametricCoefficientContext,
        expected_branch: Arc<ResidualAffineBranchSystemCertificate>,
        authorization: ResidualAffineBranchSystemFreshGuardAuthorization,
        limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> Result<Self, ResidualAffineBranchGuardCompositionError> {
        compile_fresh_sealed_guard(context, expected_branch, authorization, limits)
    }

    pub(crate) fn authenticate_with_branch_memory(
        &self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
        branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        authenticate_sealed_guard_bundle(self, context, expected_branch, branch_memory).map(|_| ())
    }

    /// Reauthenticate the complete sealed bundle and return only the
    /// source-neutral semantic projection needed by the generated-affine V2
    /// inventory.  The returned value cannot reach the raw entry, condition,
    /// provenance set, branch, Boolean cover, integer system, or an owning
    /// `Arc`.
    pub(crate) fn authenticated_source_view<'source>(
        &'source self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
        branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> Result<
        ResidualAffineBranchSealedGuardSourceView<'source>,
        ResidualAffineBranchGuardCompositionError,
    > {
        self.authenticate_with_branch_memory(context, expected_branch, branch_memory)?;
        Ok(ResidualAffineBranchSealedGuardSourceView { core: &self.core })
    }

    /// Recompute and authenticate the equal-payload comparison census while
    /// retaining every source allocation behind this opaque owner.
    pub(crate) fn authenticated_payload_comparison_census(
        &self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
        branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> Result<
        ResidualAffineBranchSealedGuardPayloadComparisonCensus,
        ResidualAffineBranchGuardCompositionError,
    > {
        self.authenticate_with_branch_memory_and_payload_comparison_census(
            context,
            expected_branch,
            branch_memory,
        )
    }

    /// Single-pass adjacent authentication plus scalar comparison census for
    /// the opaque initial-terminal owner.  This avoids replay parents first
    /// authenticating the same guard and then reauthenticating it merely to
    /// recover already-verified scalar work.
    pub(crate) fn authenticate_with_branch_memory_and_payload_comparison_census(
        &self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
        branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> Result<
        ResidualAffineBranchSealedGuardPayloadComparisonCensus,
        ResidualAffineBranchGuardCompositionError,
    > {
        let census =
            authenticate_sealed_guard_bundle(self, context, expected_branch, branch_memory)?;
        Ok(ResidualAffineBranchSealedGuardPayloadComparisonCensus {
            units: census.units,
            bytes: census.bytes,
            integer_bits: census.integer_bits,
        })
    }

    /// Checked bool-only comparison of the complete private sealed payload.
    /// Exact source-cover identity is required; branches are recursively
    /// compared so hidden diagnostics, row lineage, and integer-system state
    /// cannot evade an inventory replay.
    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, ResidualAffineBranchGuardCompositionError> {
        if Arc::ptr_eq(&self.core, &other.core) {
            return Ok(true);
        }
        let mut budget = PayloadComparisonBudget::new(self.core.limits);
        sealed_guard_payload_operand_census(&self.core, &mut budget)?;
        sealed_guard_payload_operand_census(&other.core, &mut budget)?;
        let self_integer = self
            .core
            .source_branch
            .integer_system_arc()
            .ok_or(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem)?;
        let other_integer = other
            .core
            .source_branch
            .integer_system_arc()
            .ok_or(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem)?;
        if self.core.schema != other.core.schema
            || self.core.family_fingerprint != other.core.family_fingerprint
            || self.core.context_fingerprint != other.core.context_fingerprint
            || self.core.entries != other.core.entries
            || self.core.first_contradiction_entry_ordinal
                != other.core.first_contradiction_entry_ordinal
            || self.core.limits != other.core.limits
            || self.core.stats != other.core.stats
            || self.core.memory != other.core.memory
            || self.core.payload_comparison_census != other.core.payload_comparison_census
            || !Arc::ptr_eq(&self.core.source_cover, &other.core.source_cover)
            || !Arc::ptr_eq(
                self.core.source_branch.source_cover(),
                &self.core.source_cover,
            )
            || !Arc::ptr_eq(
                other.core.source_branch.source_cover(),
                &other.core.source_cover,
            )
            || !Arc::ptr_eq(self_integer, &self.core.integer_system)
            || !Arc::ptr_eq(other_integer, &other.core.integer_system)
        {
            return Ok(false);
        }
        self.core
            .source_branch
            .payload_eq_checked(&other.core.source_branch)
            .map_err(ResidualAffineBranchGuardCompositionError::Branch)
    }

    #[cfg(test)]
    pub(crate) fn every_origin_is_generated_affine_sealed_for_test(&self) -> bool {
        self.core.entries.iter().all(|entry| {
            entry.class.condition().is_none_or(|condition| {
                condition.origins()
                    == &std::collections::BTreeSet::from([
                        GuardOrigin::GeneratedAffineSealedCondition,
                    ])
            })
        })
    }

    #[cfg(test)]
    pub(crate) fn projected_view_matches_private_for_test(
        &self,
        context: &ParametricCoefficientContext,
        expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
        branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
    ) -> bool {
        let Ok(view) = self.authenticated_source_view(context, expected_branch, branch_memory)
        else {
            return false;
        };
        view.guard_count() == self.core.entries.len()
            && view.first_contradiction_entry_ordinal()
                == self.core.first_contradiction_entry_ordinal
            && self.core.entries.iter().enumerate().all(|(position, raw)| {
                let Some(projected) = view.guard_entry(position) else {
                    return false;
                };
                let class_matches = match (raw.class(), projected.class()) {
                    (
                        ResidualAffineBranchGuardCompositionClass::Contradiction,
                        ResidualAffineBranchSealedGuardClassSourceView::Contradiction,
                    )
                    | (
                        ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant,
                        ResidualAffineBranchSealedGuardClassSourceView::DischargedNonzeroIntegerConstant,
                    ) => true,
                    (
                        ResidualAffineBranchGuardCompositionClass::BaseAssumption(raw),
                        ResidualAffineBranchSealedGuardClassSourceView::BaseAssumption {
                            condition_polynomial,
                        },
                    )
                    | (
                        ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(raw),
                        ResidualAffineBranchSealedGuardClassSourceView::FreeIndexDependent {
                            condition_polynomial,
                        },
                    ) => raw.polynomial() == condition_polynomial,
                    _ => false,
                };
                projected.structural_locus_ordinal() == raw.structural_locus_ordinal()
                    && projected.mapped_polynomial() == raw.mapped_polynomial()
                    && projected.composition_stats() == raw.composition_stats()
                    && class_matches
            })
            && view.guard_entry(self.core.entries.len()).is_none()
    }

    #[cfg(test)]
    pub(crate) fn allocations_match_for_test(
        &self,
        cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
        branch: &Arc<ResidualAffineBranchSystemCertificate>,
        integer_system: &Arc<crate::ResidualAffineIntegerSystemCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.core.source_cover, cover)
            && Arc::ptr_eq(&self.core.source_branch, branch)
            && Arc::ptr_eq(&self.core.integer_system, integer_system)
    }

    #[cfg(test)]
    pub(crate) fn payload_comparison_census_for_test(&self) -> (usize, usize, usize) {
        (
            self.core.payload_comparison_census.units,
            self.core.payload_comparison_census.bytes,
            self.core.payload_comparison_census.integer_bits,
        )
    }

    #[cfg(test)]
    pub(crate) fn tamper_source_branch_for_test(
        &mut self,
        branch: Arc<ResidualAffineBranchSystemCertificate>,
    ) {
        Arc::get_mut(&mut self.core)
            .expect("test bundle is uniquely owned")
            .source_branch = branch;
    }

    #[cfg(test)]
    pub(crate) fn tamper_memory_for_test(&mut self) {
        let core = Arc::get_mut(&mut self.core).expect("test bundle is uniquely owned");
        core.memory.retained_owned_logical_bytes =
            core.memory.retained_owned_logical_bytes.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) fn tamper_coherent_plan_memory_for_test(&mut self) {
        let core = Arc::get_mut(&mut self.core).expect("test bundle is uniquely owned");
        let plan_retained = core
            .memory
            .plan_retained_owned_logical_bytes
            .checked_add(1)
            .expect("test plan retained census fits");
        let plan_peak = core
            .memory
            .plan_compilation_owned_logical_peak_upper_bound
            .checked_add(1)
            .expect("test plan peak census fits");
        // This is deliberately coherent under the old self-fed verifier: all
        // derived maxima are rebuilt from the forged plan scalars. The new
        // structural re-census must still reject it.
        core.memory =
            sealed_guard_logical_memory_census(&core.entries, plan_retained, plan_peak, core.stats)
                .expect("test memory re-census");
    }

    #[cfg(test)]
    pub(crate) fn tamper_payload_units_for_test(&mut self) {
        let core = Arc::get_mut(&mut self.core).expect("test bundle is uniquely owned");
        core.payload_comparison_census.units =
            core.payload_comparison_census.units.saturating_add(1);
    }
}

/// Source-neutral projection of one class in the sealed V2 guard bundle.
/// Symbolic classes expose only their exact polynomial; in particular the
/// `ParametricNonZeroCondition` and its origin set stay private.
#[derive(Clone, Copy)]
pub(crate) enum ResidualAffineBranchSealedGuardClassSourceView<'source> {
    Contradiction,
    DischargedNonzeroIntegerConstant,
    BaseAssumption {
        condition_polynomial: &'source ParametricPolynomial,
    },
    FreeIndexDependent {
        condition_polynomial: &'source ParametricPolynomial,
    },
}

impl<'source> ResidualAffineBranchSealedGuardClassSourceView<'source> {
    pub(crate) const fn condition_polynomial(self) -> Option<&'source ParametricPolynomial> {
        match self {
            Self::BaseAssumption {
                condition_polynomial,
            }
            | Self::FreeIndexDependent {
                condition_polynomial,
            } => Some(condition_polynomial),
            Self::Contradiction | Self::DischargedNonzeroIntegerConstant => None,
        }
    }
}

impl fmt::Debug for ResidualAffineBranchSealedGuardClassSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Self::Contradiction => "Contradiction",
            Self::DischargedNonzeroIntegerConstant => "DischargedNonzeroIntegerConstant",
            Self::BaseAssumption { .. } => "BaseAssumption",
            Self::FreeIndexDependent { .. } => "FreeIndexDependent",
        };
        formatter
            .debug_struct("ResidualAffineBranchSealedGuardClassSourceView")
            .field("kind", &kind)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// One positional source-neutral guard projection.  This is deliberately a
/// value projection rather than a reference to the public V1 entry type.
#[derive(Clone, Copy)]
pub(crate) struct ResidualAffineBranchSealedGuardEntrySourceView<'source> {
    structural_locus_ordinal: usize,
    mapped_polynomial: &'source ParametricPolynomial,
    composition_stats: ResidualUnitAffinePolynomialCompositionStats,
    class: ResidualAffineBranchSealedGuardClassSourceView<'source>,
}

impl<'source> ResidualAffineBranchSealedGuardEntrySourceView<'source> {
    pub(crate) const fn structural_locus_ordinal(self) -> usize {
        self.structural_locus_ordinal
    }

    pub(crate) const fn mapped_polynomial(self) -> &'source ParametricPolynomial {
        self.mapped_polynomial
    }

    pub(crate) const fn composition_stats(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.composition_stats
    }

    pub(crate) const fn class(self) -> ResidualAffineBranchSealedGuardClassSourceView<'source> {
        self.class
    }
}

impl fmt::Debug for ResidualAffineBranchSealedGuardEntrySourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSealedGuardEntrySourceView")
            .field("structural_locus_ordinal", &self.structural_locus_ordinal)
            .field("composition_stats", &self.composition_stats)
            .field("class", &self.class)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

/// Lifetime-bound view of the complete positional sealed guard sequence.
#[derive(Clone, Copy)]
pub(crate) struct ResidualAffineBranchSealedGuardSourceView<'source> {
    core: &'source ResidualAffineBranchSealedGuardCore,
}

impl<'source> ResidualAffineBranchSealedGuardSourceView<'source> {
    pub(crate) const fn guard_count(self) -> usize {
        self.core.entries.len()
    }

    pub(crate) const fn first_contradiction_entry_ordinal(self) -> Option<usize> {
        self.core.first_contradiction_entry_ordinal
    }

    pub(crate) fn guard_entry(
        self,
        position: usize,
    ) -> Option<ResidualAffineBranchSealedGuardEntrySourceView<'source>> {
        let entry = self.core.entries.get(position)?;
        let class = match entry.class() {
            ResidualAffineBranchGuardCompositionClass::Contradiction => {
                ResidualAffineBranchSealedGuardClassSourceView::Contradiction
            }
            ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant => {
                ResidualAffineBranchSealedGuardClassSourceView::DischargedNonzeroIntegerConstant
            }
            ResidualAffineBranchGuardCompositionClass::BaseAssumption(condition) => {
                ResidualAffineBranchSealedGuardClassSourceView::BaseAssumption {
                    condition_polynomial: condition.polynomial(),
                }
            }
            ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(condition) => {
                ResidualAffineBranchSealedGuardClassSourceView::FreeIndexDependent {
                    condition_polynomial: condition.polynomial(),
                }
            }
        };
        Some(ResidualAffineBranchSealedGuardEntrySourceView {
            structural_locus_ordinal: entry.structural_locus_ordinal(),
            mapped_polynomial: entry.mapped_polynomial(),
            composition_stats: entry.composition_stats(),
            class,
        })
    }
}

impl fmt::Debug for ResidualAffineBranchSealedGuardSourceView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineBranchSealedGuardSourceView")
            .field("guard_count", &self.guard_count())
            .field(
                "first_contradiction_entry_ordinal",
                &self.first_contradiction_entry_ordinal(),
            )
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl ResidualAffineBranchGuardCompositionCertificate {
    /// Compose all original branch guards.  The fresh seam requires the cover
    /// to be the exact allocation already retained by `source_branch`.
    pub fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
        source_branch: Arc<ResidualAffineBranchSystemCertificate>,
        limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> Result<Self, ResidualAffineBranchGuardCompositionError> {
        catch_unwind(AssertUnwindSafe(|| {
            preflight_fresh_sources(family, context, &source_cover, &source_branch, limits)?;
            source_cover.replay(family, context)?;
            source_branch.replay_with_cover(family, context, source_cover.clone())?;

            let retained_cover = source_cover.clone();
            let retained_branch = source_branch.clone();
            let certificate =
                compile_replayed(family, context, source_cover, source_branch, limits)?;
            if !Arc::ptr_eq(&certificate.source_cover, &retained_cover) {
                return Err(
                    ResidualAffineBranchGuardCompositionError::FreshSourceCoverAllocationMismatch,
                );
            }
            if !Arc::ptr_eq(&certificate.source_branch, &retained_branch) {
                return Err(
                    ResidualAffineBranchGuardCompositionError::FreshSourceBranchAllocationMismatch,
                );
            }
            Ok(certificate)
        }))
        .map_err(|_| ResidualAffineBranchGuardCompositionError::SymbolicaPanic)?
    }

    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub const fn source_cover(&self) -> &Arc<ResidualProductLocusBooleanCoverCertificate> {
        &self.source_cover
    }

    pub const fn source_branch(&self) -> &Arc<ResidualAffineBranchSystemCertificate> {
        &self.source_branch
    }

    pub fn entries(&self) -> &[ResidualAffineBranchGuardCompositionEntry] {
        &self.entries
    }

    pub const fn limits(&self) -> ResidualAffineBranchGuardCompositionLimits {
        self.limits
    }

    pub const fn stats(&self) -> ResidualAffineBranchGuardCompositionStats {
        self.stats
    }

    pub const fn has_contradiction(&self) -> bool {
        self.first_contradiction_entry_ordinal.is_some()
    }

    pub const fn first_contradiction_entry_ordinal(&self) -> Option<usize> {
        self.first_contradiction_entry_ordinal
    }

    pub fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        self.replay_with_sources(
            family,
            context,
            self.source_cover.clone(),
            self.source_branch.clone(),
        )
    }

    /// Replay against independently allocated top-level source certificates.
    ///
    /// The supplied cover is first proven equal to the cover retained by the
    /// supplied branch. Rebuilding then uses that branch's exact cover
    /// allocation, preserving the branch fresh-seam invariant while allowing
    /// an equal independently allocated top-level `source_cover` argument.
    pub fn replay_with_sources(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
        source_branch: Arc<ResidualAffineBranchSystemCertificate>,
    ) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        catch_unwind(AssertUnwindSafe(|| {
            validate_scope(
                self.schema,
                &self.family_fingerprint,
                &self.context_fingerprint,
                family,
                context,
                self.limits,
            )?;
            preflight_replay_sources(family, context, &source_cover, &source_branch, self.limits)?;

            source_cover.replay(family, context)?;
            let branch_cover = source_branch.source_cover().clone();
            if !Arc::ptr_eq(&source_cover, &branch_cover)
                && !source_cover.payload_eq_checked(&branch_cover)?
            {
                return Err(ResidualAffineBranchGuardCompositionError::BranchSourceCoverMismatch);
            }
            source_branch.replay_with_cover(family, context, branch_cover.clone())?;

            if !Arc::ptr_eq(&self.source_cover, &source_cover)
                && !self.source_cover.payload_eq_checked(&source_cover)?
            {
                return Err(ResidualAffineBranchGuardCompositionError::SourceCoverMismatch);
            }
            if !Arc::ptr_eq(&self.source_branch, &source_branch)
                && !self.source_branch.payload_eq_checked(&source_branch)?
            {
                return Err(ResidualAffineBranchGuardCompositionError::SourceBranchMismatch);
            }

            let rebuilt =
                compile_replayed(family, context, branch_cover, source_branch, self.limits)?;
            if self.payload_eq_checked_inner(&rebuilt, true, true)? {
                Ok(())
            } else {
                Err(ResidualAffineBranchGuardCompositionError::ReplayMismatch)
            }
        }))
        .map_err(|_| ResidualAffineBranchGuardCompositionError::SymbolicaPanic)?
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, ResidualAffineBranchGuardCompositionError> {
        self.payload_eq_checked_inner(other, false, false)
    }

    fn payload_eq_checked_inner(
        &self,
        other: &Self,
        source_cover_already_equal: bool,
        source_branch_already_equal: bool,
    ) -> Result<bool, ResidualAffineBranchGuardCompositionError> {
        if std::ptr::eq(self, other) {
            return Ok(true);
        }
        preflight_payload_comparison(self, other)?;
        let local_equal = self.schema == other.schema
            && self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.entries == other.entries
            && self.first_contradiction_entry_ordinal == other.first_contradiction_entry_ordinal
            && self.limits == other.limits
            && self.stats == other.stats;
        if !local_equal {
            return Ok(false);
        }
        let covers_equal = source_cover_already_equal
            || Arc::ptr_eq(&self.source_cover, &other.source_cover)
            || self.source_cover.payload_eq_checked(&other.source_cover)?;
        if !covers_equal {
            return Ok(false);
        }
        Ok(source_branch_already_equal
            || Arc::ptr_eq(&self.source_branch, &other.source_branch)
            || self
                .source_branch
                .payload_eq_checked(&other.source_branch)?)
    }
}

#[cfg(test)]
impl ResidualAffineBranchGuardCompositionCertificate {
    pub(crate) fn tamper_first_entry_ordinal_for_test(&mut self) {
        if let Some(first) = self.entries.first_mut() {
            first.structural_locus_ordinal = usize::MAX;
        }
    }

    pub(crate) fn tamper_first_contradiction_for_test(&mut self) {
        self.first_contradiction_entry_ordinal = Some(usize::MAX);
    }

    pub(crate) fn tamper_limits_for_test(&mut self) {
        self.limits.max_guards = 0;
    }
}

/// Typed construction, scope, resource, child-certificate, and replay
/// failures for complete branch-guard composition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualAffineBranchGuardCompositionError {
    SchemaMismatch,
    WrongFamily,
    WrongContext,
    BranchSourceCoverAllocationMismatch,
    FreshSourceCoverAllocationMismatch,
    FreshSourceBranchAllocationMismatch,
    SourceCoverMismatch,
    SourceBranchMismatch,
    BranchSourceCoverMismatch,
    BranchOutcomeNotGuardedAffineMap,
    MissingIntegerSystem,
    CompositionPlanIntegerSystemAllocationMismatch,
    UnsealedGuardOrigin,
    FreshAdjacentCensusMismatch,
    StructuralLocusOrdinalOutOfRange {
        ordinal: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    AllocationFailure {
        resource: &'static str,
    },
    ReplayMismatch,
    SymbolicaPanic,
    BooleanCover(ResidualProductLocusBooleanCoverError),
    Branch(ResidualAffineBranchSystemError),
    Composition(ResidualUnitAffineCompositionError),
    Coefficient(ParametricCoefficientError),
}

impl fmt::Display for ResidualAffineBranchGuardCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("residual affine branch-guard composition schema mismatch")
            }
            Self::WrongFamily => formatter
                .write_str("residual affine branch-guard composition belongs to another family"),
            Self::WrongContext => formatter.write_str(
                "residual affine branch-guard composition belongs to another K(n) context",
            ),
            Self::BranchSourceCoverAllocationMismatch => formatter.write_str(
                "fresh branch-guard composition requires the branch's exact Boolean-cover allocation",
            ),
            Self::FreshSourceCoverAllocationMismatch => formatter.write_str(
                "fresh branch-guard composition did not retain the exact supplied Boolean-cover allocation",
            ),
            Self::FreshSourceBranchAllocationMismatch => formatter.write_str(
                "fresh branch-guard composition did not retain the exact supplied branch allocation",
            ),
            Self::SourceCoverMismatch => formatter
                .write_str("branch-guard composition source Boolean cover differs"),
            Self::SourceBranchMismatch => {
                formatter.write_str("branch-guard composition source branch differs")
            }
            Self::BranchSourceCoverMismatch => formatter.write_str(
                "supplied branch and supplied Boolean cover do not authenticate the same source",
            ),
            Self::BranchOutcomeNotGuardedAffineMap => formatter.write_str(
                "branch-guard composition requires a complete guarded affine-map outcome",
            ),
            Self::MissingIntegerSystem => formatter.write_str(
                "guarded affine branch did not expose its authenticated integer system",
            ),
            Self::CompositionPlanIntegerSystemAllocationMismatch => formatter.write_str(
                "source-neutral composition plan did not retain the branch's exact integer-system allocation",
            ),
            Self::UnsealedGuardOrigin => formatter.write_str(
                "source-neutral sealed guard retained a legacy or multi-origin condition",
            ),
            Self::FreshAdjacentCensusMismatch => formatter.write_str(
                "source-neutral sealed guard adjacent logical-memory census differs",
            ),
            Self::StructuralLocusOrdinalOutOfRange { ordinal } => write!(
                formatter,
                "Coverage V4 structural locus ordinal {ordinal} is out of range"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "residual affine branch-guard {resource} requested {requested}, configured limit is {limit}"
            ),
            Self::ResourceCountOverflow { resource } => write!(
                formatter,
                "residual affine branch-guard {resource} count overflowed usize"
            ),
            Self::AllocationFailure { resource } => write!(
                formatter,
                "could not reserve bounded residual affine branch-guard storage for {resource}"
            ),
            Self::ReplayMismatch => {
                formatter.write_str("residual affine branch-guard composition did not replay")
            }
            Self::SymbolicaPanic => formatter.write_str(
                "Symbolica panicked during residual affine branch-guard composition",
            ),
            Self::BooleanCover(source) => source.fmt(formatter),
            Self::Branch(source) => source.fmt(formatter),
            Self::Composition(source) => source.fmt(formatter),
            Self::Coefficient(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for ResidualAffineBranchGuardCompositionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BooleanCover(source) => Some(source),
            Self::Branch(source) => Some(source),
            Self::Composition(source) => Some(source),
            Self::Coefficient(source) => Some(source),
            _ => None,
        }
    }
}

impl From<ResidualProductLocusBooleanCoverError> for ResidualAffineBranchGuardCompositionError {
    fn from(value: ResidualProductLocusBooleanCoverError) -> Self {
        Self::BooleanCover(value)
    }
}

impl From<ResidualAffineBranchSystemError> for ResidualAffineBranchGuardCompositionError {
    fn from(value: ResidualAffineBranchSystemError) -> Self {
        Self::Branch(value)
    }
}

impl From<ResidualUnitAffineCompositionError> for ResidualAffineBranchGuardCompositionError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<ParametricCoefficientError> for ResidualAffineBranchGuardCompositionError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PolynomialShape {
    terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

#[derive(Clone, Copy, Debug)]
struct BranchGuardOriginLocator {
    source_case: u64,
    source_work_item_ordinal: usize,
    ready_terminal_ordinal: usize,
}

#[derive(Clone, Copy, Debug)]
enum BranchGuardOriginMode {
    Legacy(BranchGuardOriginLocator),
    GeneratedAffineSealedCondition,
}

#[derive(Debug)]
struct ComposedGuardEntries {
    entries: Vec<ResidualAffineBranchGuardCompositionEntry>,
    first_contradiction_entry_ordinal: Option<usize>,
}

fn preflight_fresh_sources(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    source_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    validate_external_scope(family, context, source_cover, source_branch, limits)?;
    if !Arc::ptr_eq(source_branch.source_cover(), source_cover) {
        return Err(ResidualAffineBranchGuardCompositionError::BranchSourceCoverAllocationMismatch);
    }
    preflight_guard_sources(context, source_cover, source_branch, limits).map(|_| ())
}

fn preflight_replay_sources(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: &Arc<ResidualProductLocusBooleanCoverCertificate>,
    source_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    validate_external_scope(family, context, source_cover, source_branch, limits)?;
    // Replay may supply an independently allocated but equal top-level cover.
    // Census against the branch's exact cover because that is the allocation
    // from which compilation will resolve every structural locus.
    preflight_guard_sources(context, source_branch.source_cover(), source_branch, limits)
        .map(|_| ())
}

fn validate_external_scope(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: &ResidualProductLocusBooleanCoverCertificate,
    source_branch: &ResidualAffineBranchSystemCertificate,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    for fingerprint in [
        family.fingerprint_ref(),
        source_cover.family_fingerprint(),
        source_branch.family_fingerprint(),
    ] {
        check_limit(
            "family fingerprint bytes",
            fingerprint.len(),
            limits.max_family_fingerprint_bytes,
        )?;
    }
    for fingerprint in [
        context.fingerprint(),
        source_cover.context_fingerprint(),
        source_branch.context_fingerprint(),
    ] {
        check_limit(
            "context fingerprint bytes",
            fingerprint.len(),
            limits.max_context_fingerprint_bytes,
        )?;
    }
    let scope_comparison_bytes = external_scope_comparison_bytes(
        family.fingerprint_ref(),
        context.fingerprint(),
        source_cover,
        source_branch,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        scope_comparison_bytes,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if family.fingerprint_ref() != source_cover.family_fingerprint()
        || family.fingerprint_ref() != source_branch.family_fingerprint()
    {
        return Err(ResidualAffineBranchGuardCompositionError::WrongFamily);
    }
    if context.fingerprint() != source_cover.context_fingerprint()
        || context.fingerprint() != source_branch.context_fingerprint()
    {
        return Err(ResidualAffineBranchGuardCompositionError::WrongContext);
    }
    Ok(())
}

fn preflight_guard_sources(
    context: &ParametricCoefficientContext,
    source_cover: &ResidualProductLocusBooleanCoverCertificate,
    source_branch: &ResidualAffineBranchSystemCertificate,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<ResidualAffineBranchGuardCompositionStats, ResidualAffineBranchGuardCompositionError> {
    if !matches!(
        source_branch.outcome(),
        ResidualAffineBranchSystemOutcome::GuardedAffineMap
    ) {
        return Err(ResidualAffineBranchGuardCompositionError::BranchOutcomeNotGuardedAffineMap);
    }
    if source_branch.integer_system_arc().is_none() {
        return Err(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem);
    }

    let ordinals = source_branch.nonzero_guard_locus_ordinals();
    check_limit("guards", ordinals.len(), limits.max_guards)?;
    check_limit(
        "structural-locus lookups",
        ordinals.len(),
        limits.max_structural_locus_lookups,
    )?;
    check_limit(
        "retained entries",
        ordinals.len(),
        limits.max_retained_entries,
    )?;

    let coverage = source_cover.source_queue().discovery().coverage();
    let mut stats = ResidualAffineBranchGuardCompositionStats {
        family_fingerprint_bytes: source_cover.family_fingerprint().len(),
        context_fingerprint_bytes: context.fingerprint().len(),
        scope_fingerprint_comparison_bytes: external_scope_comparison_bytes(
            source_cover.family_fingerprint(),
            context.fingerprint(),
            source_cover,
            source_branch,
        )?,
        guards: ordinals.len(),
        structural_locus_lookups: ordinals.len(),
        ..ResidualAffineBranchGuardCompositionStats::default()
    };
    for &ordinal in ordinals {
        let source = coverage.structural_locus(ordinal).ok_or(
            ResidualAffineBranchGuardCompositionError::StructuralLocusOrdinalOutOfRange { ordinal },
        )?;
        let terms = source.raw().nterms();
        let exponent_entries = source.raw().exponents.len();
        stats.total_source_terms = bounded_add(
            "total source terms",
            stats.total_source_terms,
            terms,
            limits.max_total_source_terms,
        )?;
        stats.total_source_exponent_entries = bounded_add(
            "total source exponent entries",
            stats.total_source_exponent_entries,
            exponent_entries,
            limits.max_total_source_exponent_entries,
        )?;
        // Charge arbitrary-precision payload incrementally. A strict branch-
        // aggregate bit limit therefore stops during this source rather than
        // after a complete untrusted coefficient scan.
        for coefficient in &source.raw().coefficients {
            stats.total_source_integer_bits = bounded_add(
                "total source integer bits",
                stats.total_source_integer_bits,
                integer_magnitude_bits(coefficient)?,
                limits.max_total_source_integer_bits,
            )?;
        }
        context
            .validate_polynomial_with_limits(source, limits.polynomial_composition.exact_algebra)?;
    }
    Ok(stats)
}

fn compile_replayed(
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    source_cover: Arc<ResidualProductLocusBooleanCoverCertificate>,
    source_branch: Arc<ResidualAffineBranchSystemCertificate>,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<
    ResidualAffineBranchGuardCompositionCertificate,
    ResidualAffineBranchGuardCompositionError,
> {
    validate_external_scope(family, context, &source_cover, &source_branch, limits)?;
    if !Arc::ptr_eq(source_branch.source_cover(), &source_cover) {
        return Err(ResidualAffineBranchGuardCompositionError::BranchSourceCoverAllocationMismatch);
    }
    let mut stats = preflight_guard_sources(context, &source_cover, &source_branch, limits)?;

    let integer_system = source_branch
        .integer_system_arc()
        .ok_or(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem)?
        .clone();
    let plan = context.compile_residual_affine_composition_plan_from_integer_system(
        integer_system.clone(),
        limits.composition_plan,
    )?;
    if !Arc::ptr_eq(plan.certificate(), &integer_system) {
        return Err(
            ResidualAffineBranchGuardCompositionError::CompositionPlanIntegerSystemAllocationMismatch,
        );
    }
    let plan_stats = plan.stats();
    stats.plan_variables = plan_stats.variables();
    stats.plan_full_images = plan_stats.full_images();
    stats.plan_geometry_entries_inspected = plan_stats.geometry_entries_inspected();
    stats.plan_geometry_entries_retained = plan_stats.geometry_entries_retained();
    stats.plan_support_entries_retained = plan_stats.support_entries_retained();
    stats.plan_total_image_terms = plan_stats.total_image_terms();
    stats.plan_total_image_exponent_entries = plan_stats.total_image_exponent_entries();
    stats.plan_largest_image_integer_bits = plan_stats.largest_image_integer_bits();
    stats.plan_total_image_integer_bits = plan_stats.total_image_integer_bits();

    let ordinals = source_branch.nonzero_guard_locus_ordinals();
    let coverage = source_cover.source_queue().discovery().coverage();
    let composed = compose_guard_entries(
        context,
        &plan,
        ordinals.len(),
        ordinals.iter().map(|&structural_locus_ordinal| {
            coverage
                .structural_locus(structural_locus_ordinal)
                .map(|source| (structural_locus_ordinal, source))
                .ok_or(
                    ResidualAffineBranchGuardCompositionError::StructuralLocusOrdinalOutOfRange {
                        ordinal: structural_locus_ordinal,
                    },
                )
        }),
        BranchGuardOriginLocator {
            source_case: source_cover.source_case().value(),
            source_work_item_ordinal: source_cover.source_work_item_ordinal(),
            ready_terminal_ordinal: source_branch.ready_terminal_ordinal(),
        },
        limits,
        &mut stats,
    )?;
    let entries = composed.entries;
    let first_contradiction_entry_ordinal = composed.first_contradiction_entry_ordinal;

    let family_fingerprint = try_copy_string(family.fingerprint_ref(), "family fingerprint")?;
    let context_fingerprint = try_copy_string(context.fingerprint(), "context fingerprint")?;
    let mut certificate = ResidualAffineBranchGuardCompositionCertificate {
        schema: RESIDUAL_AFFINE_BRANCH_GUARD_COMPOSITION_V1_SCHEMA,
        family_fingerprint,
        context_fingerprint,
        source_cover,
        source_branch,
        entries,
        first_contradiction_entry_ordinal,
        limits,
        stats,
    };
    authenticate_payload_comparison_stats(&mut certificate)?;
    Ok(certificate)
}

fn compile_fresh_sealed_guard(
    context: &ParametricCoefficientContext,
    expected_branch: Arc<ResidualAffineBranchSystemCertificate>,
    authorization: ResidualAffineBranchSystemFreshGuardAuthorization,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<ResidualAffineBranchSealedGuardBundle, ResidualAffineBranchGuardCompositionError> {
    catch_unwind(AssertUnwindSafe(|| {
        let memory_envelope = sealed_guard_memory_envelope_parts_from_limits(limits)?;
        let fresh_sources = authorization
            .into_authenticated_guard_sources(context, &expected_branch)
            .map_err(ResidualAffineBranchGuardCompositionError::Branch)?;
        if !Arc::ptr_eq(&fresh_sources.branch, &expected_branch) {
            return Err(
                ResidualAffineBranchGuardCompositionError::FreshSourceBranchAllocationMismatch,
            );
        }
        if !Arc::ptr_eq(fresh_sources.branch.source_cover(), &fresh_sources.source_cover) {
            return Err(
                ResidualAffineBranchGuardCompositionError::BranchSourceCoverAllocationMismatch,
            );
        }
        let mut stats = preflight_guard_sources(
            context,
            &fresh_sources.source_cover,
            &fresh_sources.branch,
            limits,
        )?;
        let integer_system = fresh_sources
            .branch
            .integer_system_arc()
            .ok_or(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem)?
            .clone();
        let plan = context.compile_residual_affine_composition_plan_from_fresh_integer_system(
            fresh_sources.integer_plan_authorization,
            limits.composition_plan,
        )?;
        if !Arc::ptr_eq(plan.certificate(), &integer_system) {
            return Err(
                ResidualAffineBranchGuardCompositionError::CompositionPlanIntegerSystemAllocationMismatch,
            );
        }
        let plan_memory = plan.recompute_logical_memory_census()?;
        let plan_stats = plan.stats();
        let (structural_plan_stats, structural_plan_memory) =
            recompute_sealed_guard_structural_plan_census(
                context,
                &integer_system,
                limits.composition_plan,
            )?;
        if plan_stats != structural_plan_stats || plan_memory != structural_plan_memory {
            return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
        }
        stats.plan_variables = plan_stats.variables();
        stats.plan_full_images = plan_stats.full_images();
        stats.plan_geometry_entries_inspected = plan_stats.geometry_entries_inspected();
        stats.plan_geometry_entries_retained = plan_stats.geometry_entries_retained();
        stats.plan_support_entries_retained = plan_stats.support_entries_retained();
        stats.plan_total_image_terms = plan_stats.total_image_terms();
        stats.plan_total_image_exponent_entries = plan_stats.total_image_exponent_entries();
        stats.plan_largest_image_integer_bits = plan_stats.largest_image_integer_bits();
        stats.plan_total_image_integer_bits = plan_stats.total_image_integer_bits();

        let ordinals = fresh_sources.branch.nonzero_guard_locus_ordinals();
        let coverage = fresh_sources
            .source_cover
            .source_queue()
            .discovery()
            .coverage();
        let composed = compose_guard_entries_with_origin_mode(
            context,
            &plan,
            ordinals.len(),
            ordinals.iter().map(|&structural_locus_ordinal| {
                coverage
                    .structural_locus(structural_locus_ordinal)
                    .map(|source| (structural_locus_ordinal, source))
                    .ok_or(
                        ResidualAffineBranchGuardCompositionError::StructuralLocusOrdinalOutOfRange {
                            ordinal: structural_locus_ordinal,
                        },
                    )
            }),
            BranchGuardOriginMode::GeneratedAffineSealedCondition,
            limits,
            &mut stats,
        )?;
        let family_fingerprint = try_copy_string(
            fresh_sources.branch.family_fingerprint(),
            "family fingerprint",
        )?;
        let context_fingerprint =
            try_copy_string(context.fingerprint(), "context fingerprint")?;
        let entries = composed.entries;
        let first_contradiction_entry_ordinal = composed.first_contradiction_entry_ordinal;
        let memory = sealed_guard_logical_memory_census(
            &entries,
            plan_memory.retained_owned_logical_bytes(),
            plan_memory.compilation_owned_logical_peak_upper_bound(),
            stats,
        )?;
        if memory.retained_owned_logical_bytes()
            > memory_envelope.guard_retained_owned_logical_bytes_upper_bound()
            || memory.plan_retained_owned_logical_bytes()
                > memory_envelope.plan_retained_owned_logical_bytes_upper_bound()
            || memory.plan_compilation_owned_logical_peak_upper_bound()
                > memory_envelope.plan_compilation_owned_logical_peak_upper_bound()
            || memory.compilation_owned_logical_peak_upper_bound()
                > memory_envelope.compilation_owned_logical_peak_upper_bound()
        {
            return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
        }
        let mut core = Arc::new(ResidualAffineBranchSealedGuardCore {
            schema: RESIDUAL_AFFINE_BRANCH_SEALED_GUARD_V2_SCHEMA,
            family_fingerprint,
            context_fingerprint,
            source_cover: fresh_sources.source_cover,
            source_branch: fresh_sources.branch,
            integer_system,
            entries,
            first_contradiction_entry_ordinal,
            limits,
            stats,
            memory,
            payload_comparison_census: PayloadComparisonCensus::default(),
        });
        let payload_comparison_census = sealed_guard_equal_payload_comparison_census(&core)?;
        let unique_core = Arc::get_mut(&mut core)
            .ok_or(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch)?;
        unique_core.stats.payload_comparison_units = payload_comparison_census.units;
        unique_core.stats.payload_comparison_bytes = payload_comparison_census.bytes;
        unique_core.stats.payload_comparison_integer_bits =
            payload_comparison_census.integer_bits;
        unique_core.payload_comparison_census = payload_comparison_census;
        Ok(ResidualAffineBranchSealedGuardBundle { core })
    }))
    .map_err(|_| ResidualAffineBranchGuardCompositionError::SymbolicaPanic)?
}

fn recompute_sealed_guard_structural_plan_census(
    context: &ParametricCoefficientContext,
    integer_system: &ResidualAffineIntegerSystemCertificate,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<
    (
        ResidualAffineCompositionPlanStats,
        ResidualAffineCompositionPlanLogicalMemoryCensus,
    ),
    ResidualAffineBranchGuardCompositionError,
> {
    #[cfg(test)]
    RESIDUAL_AFFINE_SEALED_GUARD_STRUCTURAL_PLAN_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    context
        .recompute_residual_affine_composition_plan_structural_census(integer_system, limits)
        .map_err(ResidualAffineBranchGuardCompositionError::Composition)
}

fn compose_guard_entries<'a>(
    context: &ParametricCoefficientContext,
    plan: &ResidualAffineCompositionPlan,
    source_count: usize,
    sources: impl IntoIterator<
        Item = Result<(usize, &'a ParametricPolynomial), ResidualAffineBranchGuardCompositionError>,
    >,
    locator: BranchGuardOriginLocator,
    limits: ResidualAffineBranchGuardCompositionLimits,
    stats: &mut ResidualAffineBranchGuardCompositionStats,
) -> Result<ComposedGuardEntries, ResidualAffineBranchGuardCompositionError> {
    compose_guard_entries_with_origin_mode(
        context,
        plan,
        source_count,
        sources,
        BranchGuardOriginMode::Legacy(locator),
        limits,
        stats,
    )
}

fn compose_guard_entries_with_origin_mode<'a>(
    context: &ParametricCoefficientContext,
    plan: &ResidualAffineCompositionPlan,
    source_count: usize,
    sources: impl IntoIterator<
        Item = Result<(usize, &'a ParametricPolynomial), ResidualAffineBranchGuardCompositionError>,
    >,
    origin_mode: BranchGuardOriginMode,
    limits: ResidualAffineBranchGuardCompositionLimits,
    stats: &mut ResidualAffineBranchGuardCompositionStats,
) -> Result<ComposedGuardEntries, ResidualAffineBranchGuardCompositionError> {
    let mut entries = Vec::new();
    entries.try_reserve_exact(source_count).map_err(|_| {
        ResidualAffineBranchGuardCompositionError::AllocationFailure {
            resource: "branch-guard composition entries",
        }
    })?;
    let mut first_contradiction_entry_ordinal = None;
    let mut composed_source_terms = 0usize;
    let mut composed_source_exponent_entries = 0usize;
    for source in sources {
        let (structural_locus_ordinal, source) = source?;
        let effective_limits = remaining_polynomial_composition_limits(
            limits,
            stats,
            composed_source_terms,
            composed_source_exponent_entries,
        )?;
        // The nested compositor has one combined integer-work cap. Obtain its
        // no-Symbolica-call preflight stats first so the compatibility-named
        // selected-backend allowance remains exact and independent of total
        // integer work.
        let preflight_stats = context.preflight_polynomial_on_residual_affine_composition_plan(
            source,
            plan,
            effective_limits,
        )?;
        check_limit(
            "total native integer-bit work",
            checked_add(
                "total native integer-bit work",
                stats.total_native_integer_bit_work,
                preflight_stats.native_integer_bit_work_bound(),
            )?,
            limits.max_total_native_integer_bit_work,
        )?;
        let composition = context.compose_polynomial_on_residual_affine_composition_plan(
            source,
            plan,
            effective_limits,
        )?;
        let (mapped_polynomial, polynomial_stats) = composition.into_parts();
        composed_source_terms = checked_add(
            "composed source terms",
            composed_source_terms,
            polynomial_stats.source_terms(),
        )?;
        composed_source_exponent_entries = checked_add(
            "composed source exponent entries",
            composed_source_exponent_entries,
            polynomial_stats.source_exponent_entries(),
        )?;
        aggregate_polynomial_stats(stats, polynomial_stats, limits)?;

        let mapped_shape = retain_polynomial_and_shape(stats, &mapped_polynomial, limits)?;
        let entry_ordinal = entries.len();
        let class = if mapped_polynomial.is_zero() {
            stats.contradictions = checked_add("contradictions", stats.contradictions, 1)?;
            if first_contradiction_entry_ordinal.is_none() {
                first_contradiction_entry_ordinal = Some(entry_ordinal);
            }
            ResidualAffineBranchGuardCompositionClass::Contradiction
        } else if mapped_polynomial.is_nonzero_constant() {
            stats.discharged_nonzero_integer_constants = checked_add(
                "discharged nonzero integer constants",
                stats.discharged_nonzero_integer_constants,
                1,
            )?;
            ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant
        } else {
            stats.retained_conditions = bounded_add(
                "retained conditions",
                stats.retained_conditions,
                1,
                limits.max_retained_conditions,
            )?;
            stats.retained_origins = bounded_add(
                "retained origins",
                stats.retained_origins,
                1,
                limits.max_retained_origins,
            )?;
            if limits.polynomial_composition.max_guard_origins < 1 {
                return Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                    resource: "per-condition guard origins",
                    requested: 1,
                    limit: limits.polynomial_composition.max_guard_origins,
                });
            }
            let origin = match origin_mode {
                BranchGuardOriginMode::Legacy(locator) => {
                    GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                        source_case: locator.source_case,
                        source_work_item_ordinal: locator.source_work_item_ordinal,
                        ready_terminal_ordinal: locator.ready_terminal_ordinal,
                        structural_locus_ordinal,
                    }
                }
                BranchGuardOriginMode::GeneratedAffineSealedCondition => {
                    GuardOrigin::GeneratedAffineSealedCondition
                }
            };
            let origin_bytes = origin.retained_byte_bound().ok_or(
                ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
                    resource: "retained origin bytes",
                },
            )?;
            stats.retained_origin_bytes = bounded_add(
                "retained origin bytes",
                stats.retained_origin_bytes,
                origin_bytes,
                limits.max_retained_origin_bytes,
            )?;

            // The entry and its semantic class deliberately retain distinct
            // authenticated polynomials. Preflight the second complete sparse
            // payload before entering its fallible allocation seam.
            retain_polynomial_shape(stats, mapped_shape, limits)?;
            let condition_polynomial = mapped_polynomial
                .try_copy_authenticated_sparse_payload()
                .map_err(|resource| {
                    ResidualAffineBranchGuardCompositionError::AllocationFailure { resource }
                })?;
            let depends_on_indices = context.polynomial_depends_on_indices_with_limits(
                &condition_polynomial,
                limits.polynomial_composition.exact_algebra,
            )?;
            let condition = context.nonzero_condition_with_origins_and_origin_limit(
                condition_polynomial,
                [origin],
                limits.polynomial_composition.exact_algebra,
                limits.polynomial_composition.max_guard_origins,
            )?;
            if depends_on_indices {
                stats.free_index_dependent_conditions = checked_add(
                    "free-index-dependent conditions",
                    stats.free_index_dependent_conditions,
                    1,
                )?;
                ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(condition)
            } else {
                stats.base_assumptions =
                    checked_add("base assumptions", stats.base_assumptions, 1)?;
                ResidualAffineBranchGuardCompositionClass::BaseAssumption(condition)
            }
        };
        entries.push(ResidualAffineBranchGuardCompositionEntry {
            structural_locus_ordinal,
            mapped_polynomial,
            class,
            polynomial_stats,
        });
    }
    stats.retained_entries = entries.len();
    Ok(ComposedGuardEntries {
        entries,
        first_contradiction_entry_ordinal,
    })
}

fn sealed_guard_logical_memory_census(
    entries: &[ResidualAffineBranchGuardCompositionEntry],
    plan_retained_owned_logical_bytes: usize,
    plan_compilation_owned_logical_peak_upper_bound: usize,
    stats: ResidualAffineBranchGuardCompositionStats,
) -> Result<
    ResidualAffineBranchSealedGuardLogicalMemoryCensus,
    ResidualAffineBranchGuardCompositionError,
> {
    #[cfg(test)]
    RESIDUAL_AFFINE_SEALED_GUARD_MEMORY_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let outer = checked_add(
        "sealed guard core logical bytes",
        sealed_guard_core_owned_logical_bytes()?,
        checked_add(
            "sealed guard fingerprint logical bytes",
            stats.family_fingerprint_bytes(),
            stats.context_fingerprint_bytes(),
        )?,
    )?;
    let mut retained_prefix = 0usize;
    let mut entry_prefix_owned_logical_peak_upper_bound = outer;
    for entry in entries {
        let retained_entry = guard_entry_retained_logical_bytes(entry)?;
        // This overlap pins the moment at which a mapped polynomial and its
        // separately copied sealed condition coexist. For condition classes
        // `retained_entry` includes both sparse payloads.
        entry_prefix_owned_logical_peak_upper_bound = entry_prefix_owned_logical_peak_upper_bound
            .max(checked_add(
                "sealed guard entry-prefix/condition-copy logical peak",
                checked_add(
                    "sealed guard entry-prefix/condition-copy logical peak",
                    outer,
                    retained_prefix,
                )?,
                retained_entry,
            )?);
        retained_prefix = checked_add(
            "sealed guard retained entry bytes",
            retained_prefix,
            retained_entry,
        )?;
    }
    let retained_owned_logical_bytes = checked_add(
        "sealed guard retained owned logical bytes",
        outer,
        retained_prefix,
    )?;
    let composition_overlap = checked_add(
        "sealed guard composition logical peak",
        plan_retained_owned_logical_bytes,
        entry_prefix_owned_logical_peak_upper_bound,
    )?;
    let compilation_owned_logical_peak_upper_bound = retained_owned_logical_bytes
        .max(plan_compilation_owned_logical_peak_upper_bound)
        .max(composition_overlap);
    Ok(ResidualAffineBranchSealedGuardLogicalMemoryCensus {
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
        plan_retained_owned_logical_bytes,
        plan_compilation_owned_logical_peak_upper_bound,
        entry_prefix_owned_logical_peak_upper_bound,
    })
}

/// Exact checked retained and plan envelope pieces derivable from sealed V2
/// limits alone. Symbolica-owned evaluator transients are excluded here and
/// admitted separately by polynomial-composition resource limits.
pub(crate) fn sealed_guard_memory_envelope_parts_from_limits(
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<
    ResidualAffineBranchSealedGuardMemoryEnvelopeParts,
    ResidualAffineBranchGuardCompositionError,
> {
    let resource = "sealed guard memory envelope";
    let plan =
        residual_affine_composition_plan_memory_envelope_from_limits(limits.composition_plan)?;
    let retained_entries = limits
        .max_guards
        .min(limits.max_structural_locus_lookups)
        .min(limits.max_retained_entries);
    // With Q=0 no entry, sparse polynomial, condition, or origin can exist.
    // Do not let irrelevant unbounded retained axes overflow a zero-manifest
    // envelope; fixed core/fingerprint storage remains charged below.
    let (
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_origin_bytes,
        retained_gmp,
    ) = if retained_entries == 0 {
        (0, 0, 0, 0)
    } else {
        (
            limits.max_retained_polynomial_terms,
            limits.max_retained_polynomial_exponent_entries,
            limits.max_retained_origin_bytes,
            guard_gmp_logical_bytes_upper_bound(
                limits.max_retained_polynomial_terms,
                limits.max_retained_polynomial_integer_bits,
            )?,
        )
    };
    let guard_retained_owned_logical_bytes_upper_bound = [
        sealed_guard_core_owned_logical_bytes()?,
        limits.max_family_fingerprint_bytes,
        limits.max_context_fingerprint_bytes,
        checked_mul(
            resource,
            retained_entries,
            size_of::<ResidualAffineBranchGuardCompositionEntry>(),
        )?,
        checked_mul(resource, retained_polynomial_terms, size_of::<Integer>())?,
        checked_mul(
            resource,
            retained_polynomial_exponent_entries,
            size_of::<u16>(),
        )?,
        retained_gmp,
        retained_origin_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| checked_add(resource, sum, bytes))?;
    let composition_peak = checked_add(
        resource,
        plan.retained_owned_logical_bytes(),
        guard_retained_owned_logical_bytes_upper_bound,
    )?;
    let compilation_owned_logical_peak_upper_bound = guard_retained_owned_logical_bytes_upper_bound
        .max(plan.compilation_owned_logical_peak_upper_bound())
        .max(composition_peak);
    Ok(ResidualAffineBranchSealedGuardMemoryEnvelopeParts {
        guard_retained_owned_logical_bytes_upper_bound,
        compilation_owned_logical_peak_upper_bound,
        plan_retained_owned_logical_bytes_upper_bound: plan.retained_owned_logical_bytes(),
        plan_compilation_owned_logical_peak_upper_bound: plan
            .compilation_owned_logical_peak_upper_bound(),
    })
}

fn authenticate_sealed_guard_bundle(
    bundle: &ResidualAffineBranchSealedGuardBundle,
    context: &ParametricCoefficientContext,
    expected_branch: &Arc<ResidualAffineBranchSystemCertificate>,
    branch_memory: ResidualAffineBranchSystemLogicalMemoryCensus,
) -> Result<PayloadComparisonCensus, ResidualAffineBranchGuardCompositionError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_SEALED_GUARD_AUTH_CALLS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let core = &bundle.core;
    if core.schema != RESIDUAL_AFFINE_BRANCH_SEALED_GUARD_V2_SCHEMA {
        return Err(ResidualAffineBranchGuardCompositionError::SchemaMismatch);
    }
    if core.family_fingerprint.len() != core.stats.family_fingerprint_bytes()
        || core.context_fingerprint.len() != core.stats.context_fingerprint_bytes()
    {
        return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
    }
    if core.context_fingerprint != context.fingerprint() {
        return Err(ResidualAffineBranchGuardCompositionError::WrongContext);
    }
    if core.family_fingerprint != expected_branch.family_fingerprint() {
        return Err(ResidualAffineBranchGuardCompositionError::WrongFamily);
    }
    if !Arc::ptr_eq(core.source_branch.source_cover(), &core.source_cover) {
        return Err(ResidualAffineBranchGuardCompositionError::BranchSourceCoverAllocationMismatch);
    }
    if !Arc::ptr_eq(&core.source_branch, expected_branch) {
        return Err(ResidualAffineBranchGuardCompositionError::FreshSourceBranchAllocationMismatch);
    }
    let branch_integer = core
        .source_branch
        .integer_system_arc()
        .ok_or(ResidualAffineBranchGuardCompositionError::MissingIntegerSystem)?;
    if !Arc::ptr_eq(branch_integer, &core.integer_system) {
        return Err(
            ResidualAffineBranchGuardCompositionError::CompositionPlanIntegerSystemAllocationMismatch,
        );
    }
    let (structural_plan_stats, structural_plan_memory) =
        recompute_sealed_guard_structural_plan_census(
            context,
            &core.integer_system,
            core.limits.composition_plan,
        )?;
    if core.stats.plan_variables() != structural_plan_stats.variables()
        || core.stats.plan_full_images() != structural_plan_stats.full_images()
        || core.stats.plan_geometry_entries_inspected()
            != structural_plan_stats.geometry_entries_inspected()
        || core.stats.plan_geometry_entries_retained()
            != structural_plan_stats.geometry_entries_retained()
        || core.stats.plan_support_entries_retained()
            != structural_plan_stats.support_entries_retained()
        || core.stats.plan_total_image_terms() != structural_plan_stats.total_image_terms()
        || core.stats.plan_total_image_exponent_entries()
            != structural_plan_stats.total_image_exponent_entries()
        || core.stats.plan_largest_image_integer_bits()
            != structural_plan_stats.largest_image_integer_bits()
        || core.stats.plan_total_image_integer_bits()
            != structural_plan_stats.total_image_integer_bits()
    {
        return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
    }
    let memory_envelope = sealed_guard_memory_envelope_parts_from_limits(core.limits)?;
    if core.memory.retained_owned_logical_bytes()
        > memory_envelope.guard_retained_owned_logical_bytes_upper_bound()
        || structural_plan_memory.retained_owned_logical_bytes()
            > memory_envelope.plan_retained_owned_logical_bytes_upper_bound()
        || structural_plan_memory.compilation_owned_logical_peak_upper_bound()
            > memory_envelope.plan_compilation_owned_logical_peak_upper_bound()
        || core.memory.compilation_owned_logical_peak_upper_bound()
            > memory_envelope.compilation_owned_logical_peak_upper_bound()
    {
        return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
    }
    if core.entries.iter().any(|entry| {
        entry.class.condition().is_some_and(|condition| {
            condition.origins().len() != 1
                || !condition
                    .origins()
                    .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        })
    }) {
        return Err(ResidualAffineBranchGuardCompositionError::UnsealedGuardOrigin);
    }
    let recomputed = sealed_guard_logical_memory_census(
        &core.entries,
        structural_plan_memory.retained_owned_logical_bytes(),
        structural_plan_memory.compilation_owned_logical_peak_upper_bound(),
        core.stats,
    )?;
    let recomputed_payload = sealed_guard_equal_payload_comparison_census(core)?;
    if recomputed != core.memory
        || recomputed_payload != core.payload_comparison_census
        || core.stats.payload_comparison_units() != core.payload_comparison_census.units
        || core.stats.payload_comparison_bytes() != core.payload_comparison_census.bytes
        || core.stats.payload_comparison_integer_bits()
            != core.payload_comparison_census.integer_bits
        || branch_memory.retained_owned_logical_bytes() == 0
        || branch_memory.compilation_owned_logical_peak_upper_bound()
            < branch_memory.retained_owned_logical_bytes()
    {
        return Err(ResidualAffineBranchGuardCompositionError::FreshAdjacentCensusMismatch);
    }
    Ok(recomputed_payload)
}

fn sealed_guard_core_owned_logical_bytes()
-> Result<usize, ResidualAffineBranchGuardCompositionError> {
    checked_add(
        "sealed guard core logical bytes",
        size_of::<ResidualAffineBranchSealedGuardBundle>(),
        checked_add(
            "sealed guard core logical bytes",
            checked_add(
                "sealed guard core logical bytes",
                checked_mul("sealed guard core logical bytes", 2, size_of::<usize>())?,
                align_of::<ResidualAffineBranchSealedGuardCore>().saturating_sub(1),
            )?,
            size_of::<ResidualAffineBranchSealedGuardCore>(),
        )?,
    )
}

fn guard_entry_retained_logical_bytes(
    entry: &ResidualAffineBranchGuardCompositionEntry,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    let mut bytes = size_of::<ResidualAffineBranchGuardCompositionEntry>();
    bytes = checked_add(
        "sealed guard retained entry bytes",
        bytes,
        guard_polynomial_dynamic_logical_bytes(&entry.mapped_polynomial)?,
    )?;
    if let Some(condition) = entry.class.condition() {
        bytes = checked_add(
            "sealed guard retained entry bytes",
            bytes,
            guard_polynomial_dynamic_logical_bytes(condition.polynomial())?,
        )?;
        for origin in condition.origins() {
            bytes = checked_add(
                "sealed guard retained entry bytes",
                bytes,
                origin.retained_byte_bound().ok_or(
                    ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
                        resource: "sealed guard origin logical bytes",
                    },
                )?,
            )?;
        }
    }
    Ok(bytes)
}

fn guard_polynomial_dynamic_logical_bytes(
    polynomial: &ParametricPolynomial,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    let raw = polynomial.raw();
    let mut bytes = checked_add(
        "sealed guard polynomial logical bytes",
        checked_mul(
            "sealed guard polynomial coefficient bytes",
            raw.coefficients.len(),
            size_of::<Integer>(),
        )?,
        checked_mul(
            "sealed guard polynomial exponent bytes",
            raw.exponents.len(),
            size_of::<u16>(),
        )?,
    )?;
    for value in &raw.coefficients {
        if matches!(value, Integer::Large(_)) {
            bytes = checked_add(
                "sealed guard polynomial logical bytes",
                bytes,
                checked_add(
                    "sealed guard large-integer logical bytes",
                    logical_bytes_for_bits(integer_magnitude_bits(value)?),
                    size_of::<usize>(),
                )?,
            )?;
        }
    }
    Ok(bytes)
}

pub(crate) fn guard_gmp_logical_bytes_upper_bound(
    integer_entries: usize,
    total_integer_bits: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    checked_add(
        "sealed guard GMP logical bytes",
        logical_bytes_for_bits(total_integer_bits),
        checked_add(
            "sealed guard GMP logical bytes",
            checked_mul(
                "sealed guard GMP logical bytes",
                integer_entries,
                size_of::<usize>(),
            )?,
            integer_entries.saturating_sub(1),
        )?,
    )
}

fn logical_bytes_for_bits(bits: usize) -> usize {
    bits / u8::BITS as usize + usize::from(bits % u8::BITS as usize != 0)
}

fn remaining_polynomial_composition_limits(
    limits: ResidualAffineBranchGuardCompositionLimits,
    stats: &ResidualAffineBranchGuardCompositionStats,
    composed_source_terms: usize,
    composed_source_exponent_entries: usize,
) -> Result<ResidualUnitAffinePolynomialCompositionLimits, ResidualAffineBranchGuardCompositionError>
{
    let mut effective = limits.polynomial_composition;
    effective.max_source_terms = effective.max_source_terms.min(remaining(
        "total source terms",
        limits.max_total_source_terms,
        composed_source_terms,
    )?);
    effective.max_source_exponent_entries = effective.max_source_exponent_entries.min(remaining(
        "total source exponent entries",
        limits.max_total_source_exponent_entries,
        composed_source_exponent_entries,
    )?);
    effective.max_expanded_contributions = effective.max_expanded_contributions.min(remaining(
        "total expanded contributions",
        limits.max_total_expanded_contributions,
        stats.total_expanded_contributions,
    )?);
    effective.max_output_terms = effective.max_output_terms.min(remaining(
        "total output-term bound",
        limits.max_total_output_term_bound,
        stats.total_output_term_bound,
    )?);
    effective.max_output_exponent_entries = effective.max_output_exponent_entries.min(remaining(
        "total output exponent-entry bound",
        limits.max_total_output_exponent_entry_bound,
        stats.total_output_exponent_entry_bound,
    )?);
    effective.max_power_calls = effective.max_power_calls.min(remaining(
        "total power calls",
        limits.max_total_power_calls,
        stats.total_power_calls,
    )?);
    effective.max_native_power_heap_pairs = effective.max_native_power_heap_pairs.min(remaining(
        "total native power heap pairs",
        limits.max_total_native_power_heap_pairs,
        stats.total_native_power_heap_pairs,
    )?);
    effective.max_multiplication_term_pairs =
        effective.max_multiplication_term_pairs.min(remaining(
            "total multiplication term pairs",
            limits.max_total_multiplication_term_pairs,
            stats.total_multiplication_term_pairs,
        )?);
    effective.max_addition_term_visits = effective.max_addition_term_visits.min(remaining(
        "total addition term visits",
        limits.max_total_addition_term_visits,
        stats.total_addition_term_visits,
    )?);
    effective.max_integer_bit_work = effective.max_integer_bit_work.min(remaining(
        "total integer-bit work",
        limits.max_total_integer_bit_work,
        stats.total_integer_bit_work,
    )?);
    Ok(effective)
}

fn remaining(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    limit
        .checked_sub(consumed)
        .ok_or(ResidualAffineBranchGuardCompositionError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        })
}

fn aggregate_polynomial_stats(
    aggregate: &mut ResidualAffineBranchGuardCompositionStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    aggregate.total_expanded_contributions = bounded_add(
        "total expanded contributions",
        aggregate.total_expanded_contributions,
        item.expanded_contribution_bound(),
        limits.max_total_expanded_contributions,
    )?;
    aggregate.total_output_term_bound = bounded_add(
        "total output-term bound",
        aggregate.total_output_term_bound,
        item.expanded_contribution_bound(),
        limits.max_total_output_term_bound,
    )?;
    aggregate.total_output_terms = bounded_add(
        "total output terms",
        aggregate.total_output_terms,
        item.output_terms(),
        limits.max_total_output_terms,
    )?;
    aggregate.total_output_exponent_entry_bound = bounded_add(
        "total output exponent-entry bound",
        aggregate.total_output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        limits.max_total_output_exponent_entry_bound,
    )?;
    aggregate.total_output_exponent_entries = bounded_add(
        "total output exponent entries",
        aggregate.total_output_exponent_entries,
        item.output_exponent_entries(),
        limits.max_total_output_exponent_entries,
    )?;
    aggregate.total_power_calls = bounded_add(
        "total power calls",
        aggregate.total_power_calls,
        item.power_calls(),
        limits.max_total_power_calls,
    )?;
    aggregate.total_native_power_heap_pairs = bounded_add(
        "total native power heap pairs",
        aggregate.total_native_power_heap_pairs,
        item.native_power_heap_pair_bound(),
        limits.max_total_native_power_heap_pairs,
    )?;
    aggregate.total_multiplication_term_pairs = bounded_add(
        "total multiplication term pairs",
        aggregate.total_multiplication_term_pairs,
        item.multiplication_term_pair_bound(),
        limits.max_total_multiplication_term_pairs,
    )?;
    aggregate.total_addition_term_visits = bounded_add(
        "total addition term visits",
        aggregate.total_addition_term_visits,
        item.addition_term_visit_bound(),
        limits.max_total_addition_term_visits,
    )?;
    aggregate.largest_kronecker_exponent_bits = aggregate
        .largest_kronecker_exponent_bits
        .max(item.largest_kronecker_exponent_bits());
    aggregate.largest_integer_coefficient_bit_bound = aggregate
        .largest_integer_coefficient_bit_bound
        .max(item.largest_integer_coefficient_bit_bound());
    aggregate.total_native_integer_bit_work = bounded_add(
        "total native integer-bit work",
        aggregate.total_native_integer_bit_work,
        item.native_integer_bit_work_bound(),
        limits.max_total_native_integer_bit_work,
    )?;
    aggregate.total_integer_bit_work = bounded_add(
        "total integer-bit work",
        aggregate.total_integer_bit_work,
        item.integer_bit_work_bound(),
        limits.max_total_integer_bit_work,
    )?;
    Ok(())
}

fn retain_polynomial_shape(
    stats: &mut ResidualAffineBranchGuardCompositionStats,
    shape: PolynomialShape,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    stats.retained_polynomial_terms = bounded_add(
        "retained polynomial terms",
        stats.retained_polynomial_terms,
        shape.terms,
        limits.max_retained_polynomial_terms,
    )?;
    stats.retained_polynomial_exponent_entries = bounded_add(
        "retained polynomial exponent entries",
        stats.retained_polynomial_exponent_entries,
        shape.exponent_entries,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    stats.retained_polynomial_integer_bits = bounded_add(
        "retained polynomial integer bits",
        stats.retained_polynomial_integer_bits,
        shape.integer_bits,
        limits.max_retained_polynomial_integer_bits,
    )?;
    Ok(())
}

fn retain_polynomial_and_shape(
    stats: &mut ResidualAffineBranchGuardCompositionStats,
    polynomial: &ParametricPolynomial,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<PolynomialShape, ResidualAffineBranchGuardCompositionError> {
    let terms = polynomial.raw().nterms();
    let exponent_entries = polynomial.raw().exponents.len();
    stats.retained_polynomial_terms = bounded_add(
        "retained polynomial terms",
        stats.retained_polynomial_terms,
        terms,
        limits.max_retained_polynomial_terms,
    )?;
    stats.retained_polynomial_exponent_entries = bounded_add(
        "retained polynomial exponent entries",
        stats.retained_polynomial_exponent_entries,
        exponent_entries,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    let mut integer_bits = 0usize;
    for coefficient in &polynomial.raw().coefficients {
        let bits = integer_magnitude_bits(coefficient)?;
        integer_bits = checked_add("retained polynomial integer bits", integer_bits, bits)?;
        stats.retained_polynomial_integer_bits = bounded_add(
            "retained polynomial integer bits",
            stats.retained_polynomial_integer_bits,
            bits,
            limits.max_retained_polynomial_integer_bits,
        )?;
    }
    Ok(PolynomialShape {
        terms,
        exponent_entries,
        integer_bits,
    })
}

fn validate_scope(
    schema: &'static str,
    family_fingerprint: &str,
    context_fingerprint: &str,
    family: &IntegralFamily,
    context: &ParametricCoefficientContext,
    limits: ResidualAffineBranchGuardCompositionLimits,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    if schema != RESIDUAL_AFFINE_BRANCH_GUARD_COMPOSITION_V1_SCHEMA {
        return Err(ResidualAffineBranchGuardCompositionError::SchemaMismatch);
    }
    let family_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        family_fingerprint,
        family.fingerprint_ref(),
        limits.max_family_fingerprint_bytes,
    )?;
    let context_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        context_fingerprint,
        context.fingerprint(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        checked_add(
            "scope fingerprint comparison bytes",
            family_bytes,
            context_bytes,
        )?,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    if family_fingerprint != family.fingerprint_ref() {
        return Err(ResidualAffineBranchGuardCompositionError::WrongFamily);
    }
    if context_fingerprint != context.fingerprint() {
        return Err(ResidualAffineBranchGuardCompositionError::WrongContext);
    }
    Ok(())
}

fn preflight_payload_comparison(
    retained: &ResidualAffineBranchGuardCompositionCertificate,
    supplied: &ResidualAffineBranchGuardCompositionCertificate,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    let limits = retained.limits;
    let family_bytes = preflight_fingerprint_pair(
        "family fingerprint bytes",
        &retained.family_fingerprint,
        &supplied.family_fingerprint,
        limits.max_family_fingerprint_bytes,
    )?;
    let context_bytes = preflight_fingerprint_pair(
        "context fingerprint bytes",
        &retained.context_fingerprint,
        &supplied.context_fingerprint,
        limits.max_context_fingerprint_bytes,
    )?;
    check_limit(
        "scope fingerprint comparison bytes",
        checked_add(
            "scope fingerprint comparison bytes",
            family_bytes,
            context_bytes,
        )?,
        limits.max_scope_fingerprint_comparison_bytes,
    )?;
    let mut budget = PayloadComparisonBudget::new(limits);
    payload_operand_census(retained, &mut budget)?;
    payload_operand_census(supplied, &mut budget)
}

fn authenticate_payload_comparison_stats(
    certificate: &mut ResidualAffineBranchGuardCompositionCertificate,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    let mut budget = PayloadComparisonBudget::new(certificate.limits);
    payload_operand_census(certificate, &mut budget)?;
    payload_operand_census(certificate, &mut budget)?;
    certificate.stats.payload_comparison_units = budget.census.units;
    certificate.stats.payload_comparison_bytes = budget.census.bytes;
    certificate.stats.payload_comparison_integer_bits = budget.census.integer_bits;
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

fn sealed_guard_equal_payload_comparison_census(
    core: &ResidualAffineBranchSealedGuardCore,
) -> Result<PayloadComparisonCensus, ResidualAffineBranchGuardCompositionError> {
    #[cfg(test)]
    RESIDUAL_AFFINE_SEALED_GUARD_LOCAL_COMPARISON_CENSUS_SCANS.with(|calls| {
        calls.set(calls.get().saturating_add(1));
    });
    let mut budget = PayloadComparisonBudget::new(core.limits);
    sealed_guard_payload_operand_census(core, &mut budget)?;
    sealed_guard_payload_operand_census(core, &mut budget)?;
    Ok(budget.census)
}

/// V2-local comparison census. Shared cover, branch, and integer-system
/// graphs stop at three Arc identity seams; no V1 certificate representation
/// or recursive source payload is charged here.
fn sealed_guard_payload_operand_census(
    core: &ResidualAffineBranchSealedGuardCore,
    budget: &mut PayloadComparisonBudget,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    budget.add_bytes(checked_add(
        "payload comparison bytes",
        size_of::<ResidualAffineBranchSealedGuardBundle>(),
        size_of::<ResidualAffineBranchSealedGuardCore>(),
    )?)?;
    budget.add_units(1)?;
    budget.add_string(core.schema)?;
    budget.add_string(&core.family_fingerprint)?;
    budget.add_string(&core.context_fingerprint)?;
    budget.add_units(3)?; // cover, branch, integer-system Arc identity seams
    budget.add_units(1)?; // entries length
    budget.add_bytes(size_of_val(core.entries.as_slice()))?;
    for entry in &core.entries {
        budget.add_units(checked_add(
            "payload comparison units",
            2,
            scalar_representation_units::<ResidualUnitAffinePolynomialCompositionStats>(),
        )?)?;
        budget.add_polynomial(&entry.mapped_polynomial)?;
        if let Some(condition) = entry.class.condition() {
            budget.add_polynomial(condition.polynomial())?;
            budget.add_units(1)?;
            for origin in condition.origins() {
                budget.charge(PayloadComparisonCensus {
                    units: 1,
                    bytes: origin.retained_byte_bound().ok_or(
                        ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
                            resource: "payload comparison origin bytes",
                        },
                    )?,
                    integer_bits: 0,
                })?;
            }
        }
    }
    budget.add_units(1)?; // first contradiction option
    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchGuardCompositionLimits,
    >())?;
    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchGuardCompositionStats,
    >())?;
    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchSealedGuardLogicalMemoryCensus,
    >())?;
    budget.add_units(scalar_representation_units::<PayloadComparisonCensus>())?;
    Ok(())
}

struct PayloadComparisonBudget {
    limits: ResidualAffineBranchGuardCompositionLimits,
    census: PayloadComparisonCensus,
}

impl PayloadComparisonBudget {
    const fn new(limits: ResidualAffineBranchGuardCompositionLimits) -> Self {
        Self {
            limits,
            census: PayloadComparisonCensus {
                units: 0,
                bytes: 0,
                integer_bits: 0,
            },
        }
    }

    fn charge(
        &mut self,
        additional: PayloadComparisonCensus,
    ) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        self.census.units = bounded_add(
            "payload comparison units",
            self.census.units,
            additional.units,
            self.limits.max_payload_comparison_units,
        )?;
        self.census.bytes = bounded_add(
            "payload comparison bytes",
            self.census.bytes,
            additional.bytes,
            self.limits.max_payload_comparison_bytes,
        )?;
        self.census.integer_bits = bounded_add(
            "payload comparison integer bits",
            self.census.integer_bits,
            additional.integer_bits,
            self.limits.max_payload_comparison_integer_bits,
        )?;
        Ok(())
    }

    fn add_units(&mut self, units: usize) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        self.charge(PayloadComparisonCensus {
            units,
            ..PayloadComparisonCensus::default()
        })
    }

    fn add_bytes(&mut self, bytes: usize) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        self.charge(PayloadComparisonCensus {
            bytes,
            ..PayloadComparisonCensus::default()
        })
    }

    fn add_string(&mut self, value: &str) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        self.charge(PayloadComparisonCensus {
            units: value.len(),
            bytes: value.len(),
            integer_bits: 0,
        })
    }

    fn add_polynomial(
        &mut self,
        polynomial: &ParametricPolynomial,
    ) -> Result<(), ResidualAffineBranchGuardCompositionError> {
        let raw = polynomial.raw();
        // Charge all length metadata and fixed sparse/variable-map backing
        // before entering any element loop. The ParametricPolynomial handle
        // itself is already part of the enclosing entry-slice byte charge.
        self.charge(PayloadComparisonCensus {
            units: checked_add(
                "payload comparison units",
                checked_add(
                    "payload comparison units",
                    checked_add("payload comparison units", 3, raw.coefficients.len())?,
                    raw.exponents.len(),
                )?,
                raw.variables.len(),
            )?,
            bytes: checked_add(
                "payload comparison bytes",
                size_of_val(raw.coefficients.as_slice()),
                checked_add(
                    "payload comparison bytes",
                    size_of_val(raw.exponents.as_slice()),
                    checked_mul(
                        "payload comparison bytes",
                        raw.variables.len(),
                        size_of::<PolyVariable>(),
                    )?,
                )?,
            )?,
            integer_bits: 0,
        })?;
        for coefficient in &raw.coefficients {
            self.charge(PayloadComparisonCensus {
                integer_bits: integer_magnitude_bits(coefficient)?,
                ..PayloadComparisonCensus::default()
            })?;
        }
        // ParametricPolynomial equality also compares its authenticated
        // context Arc<str>. Charge the Arc seam and complete string payload.
        self.add_units(1)?;
        self.add_string(polynomial.authenticated_context_fingerprint())
    }
}

/// Recompute only certificate-owned equality work. The source cover and
/// source branch stop at their `Arc`/checked-comparator seams and enforce
/// independent nested limits when those seams are crossed.
fn payload_operand_census(
    certificate: &ResidualAffineBranchGuardCompositionCertificate,
    budget: &mut PayloadComparisonBudget,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    budget.add_bytes(size_of::<ResidualAffineBranchGuardCompositionCertificate>())?;
    budget.add_units(1)?;
    budget.add_string(certificate.schema)?;
    budget.add_string(&certificate.family_fingerprint)?;
    budget.add_string(&certificate.context_fingerprint)?;
    budget.add_units(2)?; // cover and branch Arc checked-comparator seams
    budget.add_units(1)?; // entries length
    budget.add_bytes(size_of_val(certificate.entries.as_slice()))?;
    for entry in &certificate.entries {
        budget.add_units(checked_add(
            "payload comparison units",
            2, // structural-locus ordinal and class discriminant
            scalar_representation_units::<ResidualUnitAffinePolynomialCompositionStats>(),
        )?)?;
        budget.add_polynomial(&entry.mapped_polynomial)?;
        if let Some(condition) = entry.class.condition() {
            budget.add_polynomial(condition.polynomial())?;
            budget.add_units(1)?; // origin-set length
            for origin in condition.origins() {
                let bytes = origin.retained_byte_bound().ok_or(
                    ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
                        resource: "payload comparison origin bytes",
                    },
                )?;
                budget.charge(PayloadComparisonCensus {
                    units: 1,
                    bytes,
                    integer_bits: 0,
                })?;
            }
        }
    }
    budget.add_units(1)?; // first contradiction option
    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchGuardCompositionLimits,
    >())?;
    budget.add_units(scalar_representation_units::<
        ResidualAffineBranchGuardCompositionStats,
    >())?;
    Ok(())
}

fn scalar_representation_units<T>() -> usize {
    let bytes = size_of::<T>();
    let word = size_of::<usize>();
    bytes / word + usize::from(bytes % word != 0)
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        }
    })
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, ResidualAffineBranchGuardCompositionError> {
    let mut target = String::new();
    target
        .try_reserve_exact(source.len())
        .map_err(|_| ResidualAffineBranchGuardCompositionError::AllocationFailure { resource })?;
    target.push_str(source);
    Ok(target)
}

fn preflight_fingerprint_pair(
    resource: &'static str,
    retained: &str,
    supplied: &str,
    one_fingerprint_limit: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    check_limit(resource, retained.len(), one_fingerprint_limit)?;
    check_limit(resource, supplied.len(), one_fingerprint_limit)?;
    checked_add(
        "scope fingerprint comparison bytes",
        retained.len(),
        supplied.len(),
    )
}

fn external_scope_comparison_bytes(
    authoritative_family: &str,
    authoritative_context: &str,
    source_cover: &ResidualProductLocusBooleanCoverCertificate,
    source_branch: &ResidualAffineBranchSystemCertificate,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    [
        (authoritative_family, source_cover.family_fingerprint()),
        (authoritative_family, source_branch.family_fingerprint()),
        (authoritative_context, source_cover.context_fingerprint()),
        (authoritative_context, source_branch.context_fingerprint()),
    ]
    .into_iter()
    .try_fold(0usize, |total, (left, right)| {
        let pair = checked_add(
            "scope fingerprint comparison bytes",
            left.len(),
            right.len(),
        )?;
        checked_add("scope fingerprint comparison bytes", total, pair)
    })
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    left.checked_add(right)
        .ok_or(ResidualAffineBranchGuardCompositionError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    left.checked_mul(right)
        .ok_or(ResidualAffineBranchGuardCompositionError::ResourceCountOverflow { resource })
}

fn bounded_add(
    resource: &'static str,
    current: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, ResidualAffineBranchGuardCompositionError> {
    let requested = checked_add(resource, current, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualAffineBranchGuardCompositionError> {
    if requested > limit {
        Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedSectorLiveLeafQueueCompiler,
        GeneratedSectorLiveLeafQueueLimits, IntegralOrderingPolicy, ParametricArithmeticLimits,
        ParametricIbpGenerator, ResidualAffineBranchSystemLimits,
        ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemInputRow,
        ResidualAffineIntegerSystemLimits, ResidualAffinePrimitiveRow,
        ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverLimits,
        ResidualProductLocusBooleanNodeOutcome, SectorMask,
    };

    fn synthetic_context(scope: &str) -> ParametricCoefficientContext {
        let base = CoefficientContext::new(["d", "m2"]);
        ParametricCoefficientContext::try_new(&base, scope, 4).unwrap()
    }

    fn synthetic_plan(
        context: &ParametricCoefficientContext,
        limits: ResidualAffineBranchGuardCompositionLimits,
    ) -> ResidualAffineCompositionPlan {
        // n0 - n1 - n2 = 0, hence F(t) = (t1+t2,t1,t2,t3).
        let row = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            [0, 1, -1, -1, 0].into_iter().map(Integer::from).collect(),
            5,
            1_000,
            10_000,
        )
        .unwrap();
        let input = ResidualAffineIntegerSystemInputRow::try_new(row, vec![101], 1).unwrap();
        let system = Arc::new(
            ResidualAffineIntegerSystemCertificate::compile(
                4,
                &[input],
                ResidualAffineIntegerSystemLimits::default(),
            )
            .unwrap(),
        );
        let map = system.affine_map().unwrap();
        assert_eq!(map.free_positions(), &[1, 2, 3]);
        context
            .compile_residual_affine_composition_plan_from_integer_system(
                system,
                limits.composition_plan,
            )
            .unwrap()
    }

    fn polynomial(
        context: &ParametricCoefficientContext,
        value: &crate::ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    fn natural_certificate() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        ResidualAffineBranchGuardCompositionCertificate,
    ) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        let family = IntegralFamily::new(
            "branch-guard-unit-tamper-sunset",
            vec!["k1".into(), "k2".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![one.clone(), zero.clone(), zero.clone()],
                ),
                AffineDenominator::new(
                    minus_m2.clone(),
                    vec![zero.clone(), zero.clone(), one.clone()],
                ),
                AffineDenominator::new(minus_m2, vec![one.clone(), coefficients.integer(2), one]),
            ],
            Vec::new(),
            vec![zero.clone(), zero.clone(), zero],
        )
        .unwrap();
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let mut queue_limits = GeneratedSectorLiveLeafQueueLimits::default();
        queue_limits.translation_radius = 0;
        queue_limits.max_translation_points = 1;
        let queue = Arc::new(
            GeneratedSectorLiveLeafQueueCompiler::compile(
                &family,
                &context,
                &discovery,
                queue_limits,
            )
            .unwrap(),
        );
        let cover = Arc::new(
            ResidualProductLocusBooleanCoverCompiler::compile(
                &family,
                &context,
                queue,
                0,
                ResidualProductLocusBooleanCoverLimits::default(),
            )
            .unwrap(),
        );
        let branch = cover
            .nodes()
            .iter()
            .filter(|node| {
                matches!(
                    node.outcome(),
                    ResidualProductLocusBooleanNodeOutcome::ReadyForAffineRecognition
                ) && !node.nonzero_atoms().is_empty()
            })
            .map(|node| {
                Arc::new(
                    ResidualAffineBranchSystemCertificate::compile(
                        &family,
                        &context,
                        cover.clone(),
                        node.ordinal(),
                        ResidualAffineBranchSystemLimits::default(),
                    )
                    .unwrap(),
                )
            })
            .find(|branch| {
                matches!(
                    branch.outcome(),
                    ResidualAffineBranchSystemOutcome::GuardedAffineMap
                )
            })
            .unwrap();
        let certificate = ResidualAffineBranchGuardCompositionCertificate::compile(
            &family,
            &context,
            cover,
            branch,
            ResidualAffineBranchGuardCompositionLimits::default(),
        )
        .unwrap();
        (family, context, certificate)
    }

    #[test]
    fn branch_guard_origin_is_flat_stable_and_allocation_free() {
        let origin = GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
            source_case: 17,
            source_work_item_ordinal: 19,
            ready_terminal_ordinal: 23,
            structural_locus_ordinal: 29,
        };
        assert_eq!(
            origin.stable_string(),
            "residual-affine-branch-nonzero-guard-substitution:17:19:23:29"
        );
        assert_eq!(
            origin.retained_byte_bound(),
            GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                source_case: 0,
                source_work_item_ordinal: 0,
                ready_terminal_ordinal: 0,
                structural_locus_ordinal: 0,
            }
            .retained_byte_bound()
        );
    }

    #[test]
    fn aggregate_default_limits_do_not_reset_to_one_polynomial() {
        let limits = ResidualAffineBranchGuardCompositionLimits::default();
        assert!(limits.max_total_source_terms > limits.polynomial_composition.max_source_terms);
        assert!(limits.max_total_output_terms > limits.polynomial_composition.max_output_terms);
        assert!(limits.max_retained_entries >= limits.max_guards);
    }

    #[test]
    fn sealed_guard_limit_memory_envelope_parts_are_exact_checked() {
        let mut zero = ResidualAffineBranchGuardCompositionLimits::default();
        zero.composition_plan = ResidualUnitAffineCompositionPlanLimits {
            max_variables: 0,
            max_full_images: 0,
            max_geometry_entries_inspected: 0,
            max_geometry_entries_retained: 0,
            max_support_entries_retained: 0,
            max_total_image_terms: 0,
            max_total_image_exponent_entries: 0,
            max_image_integer_bits: 0,
            max_total_image_integer_bits: 0,
        };
        zero.max_family_fingerprint_bytes = 0;
        zero.max_context_fingerprint_bytes = 0;
        zero.max_guards = 0;
        zero.max_structural_locus_lookups = 0;
        zero.max_retained_entries = 0;
        zero.max_retained_polynomial_terms = 0;
        zero.max_retained_polynomial_exponent_entries = 0;
        zero.max_retained_polynomial_integer_bits = 0;
        zero.max_retained_origin_bytes = 0;

        let parts = sealed_guard_memory_envelope_parts_from_limits(zero).unwrap();
        let plan =
            residual_affine_composition_plan_memory_envelope_from_limits(zero.composition_plan)
                .unwrap();
        assert_eq!(
            parts.guard_retained_owned_logical_bytes_upper_bound(),
            sealed_guard_core_owned_logical_bytes().unwrap()
        );
        assert_eq!(
            parts.plan_retained_owned_logical_bytes_upper_bound(),
            plan.retained_owned_logical_bytes()
        );
        assert_eq!(
            parts.plan_compilation_owned_logical_peak_upper_bound(),
            plan.compilation_owned_logical_peak_upper_bound()
        );
        assert!(
            parts.plan_compilation_owned_logical_peak_upper_bound()
                >= parts.plan_retained_owned_logical_bytes_upper_bound()
        );

        let mut irrelevant_zero_axes = zero;
        irrelevant_zero_axes.max_retained_polynomial_terms = usize::MAX;
        irrelevant_zero_axes.max_retained_polynomial_exponent_entries = usize::MAX;
        irrelevant_zero_axes.max_retained_polynomial_integer_bits = usize::MAX;
        irrelevant_zero_axes.max_retained_origin_bytes = usize::MAX;
        irrelevant_zero_axes
            .polynomial_composition
            .max_expanded_contributions = usize::MAX;
        irrelevant_zero_axes
            .polynomial_composition
            .max_native_power_heap_pairs = usize::MAX;
        irrelevant_zero_axes
            .polynomial_composition
            .max_integer_coefficient_bits = usize::MAX;
        assert_eq!(
            sealed_guard_memory_envelope_parts_from_limits(irrelevant_zero_axes).unwrap(),
            parts,
            "Q=0 must ignore unreachable retained maxima"
        );

        let mut overflow = zero;
        overflow.max_guards = usize::MAX;
        overflow.max_structural_locus_lookups = usize::MAX;
        overflow.max_retained_entries = usize::MAX;
        assert!(matches!(
            sealed_guard_memory_envelope_parts_from_limits(overflow),
            Err(
                ResidualAffineBranchGuardCompositionError::ResourceCountOverflow {
                    resource: "sealed guard memory envelope"
                }
            )
        ));
    }

    #[test]
    fn synthetic_shared_plan_classifies_all_guards_without_short_circuiting() {
        let context = synthetic_context("branch-guard-synthetic-all-classes");
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let n3 = context.index(3).unwrap();
        let n1_plus_n2 = context.add(&n1, &n2).unwrap();
        let equality = context.sub(&n0, &n1_plus_n2).unwrap();
        let seven_after_equality = context.add(&equality, &context.integer(7)).unwrap();
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let free_first = context.add(&n0, &n3).unwrap();
        let free_duplicate = context.add(&n1_plus_n2, &n3).unwrap();
        let sources = vec![
            (11, polynomial(&context, &equality)),
            (13, polynomial(&context, &seven_after_equality)),
            (17, polynomial(&context, &d)),
            (19, polynomial(&context, &free_first)),
            (23, polynomial(&context, &free_duplicate)),
        ];
        let limits = ResidualAffineBranchGuardCompositionLimits::default();
        let plan = synthetic_plan(&context, limits);
        let mut stats = ResidualAffineBranchGuardCompositionStats::default();
        let composed = compose_guard_entries(
            &context,
            &plan,
            sources.len(),
            sources
                .iter()
                .map(|(ordinal, source)| Ok((*ordinal, source))),
            BranchGuardOriginLocator {
                source_case: 29,
                source_work_item_ordinal: 31,
                ready_terminal_ordinal: 37,
            },
            limits,
            &mut stats,
        )
        .unwrap();

        assert_eq!(composed.entries.len(), sources.len());
        assert_eq!(composed.first_contradiction_entry_ordinal, Some(0));
        assert!(matches!(
            composed.entries[0].class(),
            ResidualAffineBranchGuardCompositionClass::Contradiction
        ));
        assert!(composed.entries[0].mapped_polynomial().is_zero());
        assert!(matches!(
            composed.entries[1].class(),
            ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant
        ));
        assert!(
            composed.entries[1]
                .mapped_polynomial()
                .is_nonzero_constant()
        );
        assert!(matches!(
            composed.entries[2].class(),
            ResidualAffineBranchGuardCompositionClass::BaseAssumption(_)
        ));
        assert!(matches!(
            composed.entries[3].class(),
            ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(_)
        ));
        assert!(matches!(
            composed.entries[4].class(),
            ResidualAffineBranchGuardCompositionClass::FreeIndexDependent(_)
        ));
        assert_eq!(
            composed.entries[3].mapped_polynomial(),
            composed.entries[4].mapped_polynomial(),
            "equal mapped guards must remain separate source entries"
        );
        let origins: Vec<_> = composed.entries[3..]
            .iter()
            .map(|entry| {
                let condition = entry.class().condition().unwrap();
                assert_eq!(condition.origins().len(), 1);
                condition.origins().iter().next().unwrap()
            })
            .collect();
        assert_ne!(origins[0], origins[1]);
        assert_eq!(
            origins[0],
            &GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                source_case: 29,
                source_work_item_ordinal: 31,
                ready_terminal_ordinal: 37,
                structural_locus_ordinal: 19,
            }
        );
        assert_eq!(
            origins[1],
            &GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
                source_case: 29,
                source_work_item_ordinal: 31,
                ready_terminal_ordinal: 37,
                structural_locus_ordinal: 23,
            }
        );
        assert_eq!(stats.contradictions(), 1);
        assert_eq!(stats.discharged_nonzero_integer_constants(), 1);
        assert_eq!(stats.base_assumptions(), 1);
        assert_eq!(stats.free_index_dependent_conditions(), 2);
        assert_eq!(stats.retained_entries(), 5);

        // Directly check G(F(t)) for a nonleading map with dependence on all
        // three free coordinates. Nonfree residual slots are irrelevant and
        // are set to an adversarial value to prove they were eliminated.
        let residual = [91, -3, 5, 7];
        let ambient = [2, -3, 5, 7];
        let arithmetic = ParametricArithmeticLimits::default();
        assert_eq!(
            context
                .specialize_polynomial(&sources[3].1, &ambient, arithmetic)
                .unwrap(),
            context
                .specialize_polynomial(
                    composed.entries[3].mapped_polynomial(),
                    &residual,
                    arithmetic,
                )
                .unwrap()
        );
    }

    #[test]
    fn sealed_v2_origin_mode_uses_symbolica_compositor_and_preserves_provenance() {
        let context = synthetic_context("sealed-v2-symbolica-native-compositor-dispatch");
        let n0 = context.index(0).unwrap();
        let source = polynomial(&context, &context.mul(&n0, &n0).unwrap());
        let limits = ResidualAffineBranchGuardCompositionLimits::default();
        let plan = synthetic_plan(&context, limits);

        let mut native_stats = ResidualAffineBranchGuardCompositionStats::default();
        let native = compose_guard_entries_with_origin_mode(
            &context,
            &plan,
            1,
            [Ok((101, &source))],
            BranchGuardOriginMode::GeneratedAffineSealedCondition,
            limits,
            &mut native_stats,
        )
        .unwrap();
        assert_eq!(native.entries.len(), 1);
        assert!(
            native.entries[0]
                .polynomial_stats()
                .largest_kronecker_exponent_bits()
                > 0
        );
        let condition = native.entries[0].class().condition().unwrap();
        assert_eq!(condition.origins().len(), 1);
        assert!(
            condition
                .origins()
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        );

        let mut legacy_stats = ResidualAffineBranchGuardCompositionStats::default();
        let legacy = compose_guard_entries(
            &context,
            &plan,
            1,
            [Ok((101, &source))],
            BranchGuardOriginLocator {
                source_case: 103,
                source_work_item_ordinal: 107,
                ready_terminal_ordinal: 109,
            },
            limits,
            &mut legacy_stats,
        )
        .unwrap();
        assert_eq!(
            native.entries[0].mapped_polynomial(),
            legacy.entries[0].mapped_polynomial()
        );
        assert_eq!(
            native.entries[0].polynomial_stats(),
            legacy.entries[0].polynomial_stats()
        );

        let mut rejected_limits = limits;
        rejected_limits
            .polynomial_composition
            .max_kronecker_exponent_bits = 0;
        let mut rejected_stats = ResidualAffineBranchGuardCompositionStats::default();
        assert!(matches!(
            compose_guard_entries_with_origin_mode(
                &context,
                &plan,
                1,
                [Ok((101, &source))],
                BranchGuardOriginMode::GeneratedAffineSealedCondition,
                rejected_limits,
                &mut rejected_stats,
            ),
            Err(ResidualAffineBranchGuardCompositionError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource: "Kronecker exponent bits",
                    requested,
                    limit: 0,
                }
            )) if requested > 0
        ));

        let plan_memory = plan.recompute_logical_memory_census().unwrap();
        let memory = sealed_guard_logical_memory_census(
            &native.entries,
            plan_memory.retained_owned_logical_bytes(),
            plan_memory.compilation_owned_logical_peak_upper_bound(),
            native_stats,
        )
        .unwrap();
        let outer = sealed_guard_core_owned_logical_bytes().unwrap();
        let retained_entry = guard_entry_retained_logical_bytes(&native.entries[0]).unwrap();
        let expected_entry_peak = outer + retained_entry;
        assert_eq!(
            memory.entry_prefix_owned_logical_peak_upper_bound(),
            expected_entry_peak
        );
        let expected_compilation_peak = (outer + retained_entry)
            .max(plan_memory.compilation_owned_logical_peak_upper_bound())
            .max(plan_memory.retained_owned_logical_bytes() + expected_entry_peak);
        assert_eq!(
            memory.compilation_owned_logical_peak_upper_bound(),
            expected_compilation_peak
        );
        let envelope = sealed_guard_memory_envelope_parts_from_limits(limits).unwrap();
        assert!(
            memory.compilation_owned_logical_peak_upper_bound()
                <= envelope.compilation_owned_logical_peak_upper_bound()
        );
    }

    #[test]
    fn synthetic_late_contradiction_retains_the_complete_guard_suffix() {
        let context = synthetic_context("branch-guard-synthetic-late-contradiction");
        let n1_plus_n2 = context
            .add(&context.index(1).unwrap(), &context.index(2).unwrap())
            .unwrap();
        let equality = context
            .sub(&context.index(0).unwrap(), &n1_plus_n2)
            .unwrap();
        let nonzero = context.add(&equality, &context.integer(7)).unwrap();
        let sources = [
            (73, polynomial(&context, &nonzero)),
            (79, polynomial(&context, &equality)),
            (83, polynomial(&context, &equality)),
        ];
        let limits = ResidualAffineBranchGuardCompositionLimits::default();
        let plan = synthetic_plan(&context, limits);
        let mut stats = ResidualAffineBranchGuardCompositionStats::default();
        let composed = compose_guard_entries(
            &context,
            &plan,
            sources.len(),
            sources
                .iter()
                .map(|(ordinal, source)| Ok((*ordinal, source))),
            BranchGuardOriginLocator {
                source_case: 89,
                source_work_item_ordinal: 97,
                ready_terminal_ordinal: 101,
            },
            limits,
            &mut stats,
        )
        .unwrap();

        assert_eq!(composed.entries.len(), 3);
        assert_eq!(composed.first_contradiction_entry_ordinal, Some(1));
        assert!(matches!(
            composed.entries[0].class(),
            ResidualAffineBranchGuardCompositionClass::DischargedNonzeroIntegerConstant
        ));
        assert!(matches!(
            composed.entries[1].class(),
            ResidualAffineBranchGuardCompositionClass::Contradiction
        ));
        assert!(matches!(
            composed.entries[2].class(),
            ResidualAffineBranchGuardCompositionClass::Contradiction
        ));
        assert_eq!(stats.contradictions(), 2);
        assert_eq!(stats.retained_entries(), 3);
    }

    #[test]
    fn gmp_coefficient_limit_is_exact_at_the_source_neutral_composition_seam() {
        let context = synthetic_context("branch-guard-synthetic-gmp-limit");
        let huge = context
            .lift(
                &context
                    .base()
                    .parse("340282366920938463463374607431768211456")
                    .unwrap(),
            )
            .unwrap(); // 2^128, hence 129 significant bits.
        let source = context.mul(&huge, &context.index(3).unwrap()).unwrap();
        let source = polynomial(&context, &source);

        let mut exact = ResidualAffineBranchGuardCompositionLimits::default();
        // The native preflight carries one conservative signed-growth bit on
        // top of the exact 129-bit source magnitude.
        exact.polynomial_composition.max_integer_coefficient_bits = 130;
        let plan = synthetic_plan(&context, exact);
        let mut stats = ResidualAffineBranchGuardCompositionStats::default();
        compose_guard_entries(
            &context,
            &plan,
            1,
            [Ok((41, &source))],
            BranchGuardOriginLocator {
                source_case: 43,
                source_work_item_ordinal: 47,
                ready_terminal_ordinal: 53,
            },
            exact,
            &mut stats,
        )
        .unwrap();

        let mut one_below = exact;
        one_below
            .polynomial_composition
            .max_integer_coefficient_bits = 129;
        let plan = synthetic_plan(&context, one_below);
        let mut stats = ResidualAffineBranchGuardCompositionStats::default();
        assert!(matches!(
            compose_guard_entries(
                &context,
                &plan,
                1,
                [Ok((41, &source))],
                BranchGuardOriginLocator {
                    source_case: 43,
                    source_work_item_ordinal: 47,
                    ready_terminal_ordinal: 53,
                },
                one_below,
                &mut stats,
            ),
            Err(ResidualAffineBranchGuardCompositionError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource: "integer coefficient bits",
                    requested: 130,
                    limit: 129,
                }
            ))
        ));
    }

    #[test]
    fn second_guard_receives_only_the_remaining_branch_aggregate_allowance() {
        let context = synthetic_context("branch-guard-synthetic-remaining-budget");
        let first = context
            .add(&context.index(0).unwrap(), &context.index(3).unwrap())
            .unwrap();
        let n1_plus_n2 = context
            .add(&context.index(1).unwrap(), &context.index(2).unwrap())
            .unwrap();
        let second = context
            .add(&n1_plus_n2, &context.index(3).unwrap())
            .unwrap();
        let sources = [polynomial(&context, &first), polynomial(&context, &second)];
        let locator = BranchGuardOriginLocator {
            source_case: 59,
            source_work_item_ordinal: 61,
            ready_terminal_ordinal: 67,
        };

        let baseline_limits = ResidualAffineBranchGuardCompositionLimits::default();
        let plan = synthetic_plan(&context, baseline_limits);
        let mut baseline_stats = ResidualAffineBranchGuardCompositionStats::default();
        compose_guard_entries(
            &context,
            &plan,
            2,
            sources
                .iter()
                .enumerate()
                .map(|(ordinal, source)| Ok((71 + ordinal, source))),
            locator,
            baseline_limits,
            &mut baseline_stats,
        )
        .unwrap();
        let exact_work = baseline_stats.total_expanded_contributions();
        assert!(exact_work >= 2);

        let mut exact = baseline_limits;
        exact.max_total_expanded_contributions = exact_work;
        let plan = synthetic_plan(&context, exact);
        let mut exact_stats = ResidualAffineBranchGuardCompositionStats::default();
        compose_guard_entries(
            &context,
            &plan,
            2,
            sources
                .iter()
                .enumerate()
                .map(|(ordinal, source)| Ok((71 + ordinal, source))),
            locator,
            exact,
            &mut exact_stats,
        )
        .unwrap();
        assert_eq!(exact_stats.total_expanded_contributions(), exact_work);

        let mut one_below = exact;
        one_below.max_total_expanded_contributions = exact_work - 1;
        let plan = synthetic_plan(&context, one_below);
        let mut partial_stats = ResidualAffineBranchGuardCompositionStats::default();
        let error = compose_guard_entries(
            &context,
            &plan,
            2,
            sources
                .iter()
                .enumerate()
                .map(|(ordinal, source)| Ok((71 + ordinal, source))),
            locator,
            one_below,
            &mut partial_stats,
        )
        .unwrap_err();
        assert!(partial_stats.total_expanded_contributions() > 0);
        assert!(partial_stats.total_expanded_contributions() < exact_work);
        assert!(
            matches!(
                error,
                ResidualAffineBranchGuardCompositionError::Composition(
                    ResidualUnitAffineCompositionError::ResourceLimit {
                        resource: "expanded polynomial contributions",
                        ..
                    }
                )
            ),
            "{error:?}"
        );
    }

    #[test]
    fn checked_comparison_and_replay_reject_one_below_budgets_and_tampering() {
        let (family, context, certificate) = natural_certificate();
        assert!(
            certificate
                .payload_eq_checked(&certificate.clone())
                .unwrap()
        );

        let pair_stats = certificate.stats();
        let mut one_below = certificate.clone();
        one_below.limits.max_payload_comparison_units = pair_stats.payload_comparison_units() - 1;
        assert!(matches!(
            one_below.payload_eq_checked(&certificate),
            Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                resource: "payload comparison units",
                ..
            })
        ));
        let mut one_below = certificate.clone();
        one_below.limits.max_payload_comparison_bytes = pair_stats.payload_comparison_bytes() - 1;
        assert!(matches!(
            one_below.payload_eq_checked(&certificate),
            Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                resource: "payload comparison bytes",
                ..
            })
        ));
        if pair_stats.payload_comparison_integer_bits() > 0 {
            let mut one_below = certificate.clone();
            one_below.limits.max_payload_comparison_integer_bits =
                pair_stats.payload_comparison_integer_bits() - 1;
            assert!(matches!(
                one_below.payload_eq_checked(&certificate),
                Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                    resource: "payload comparison integer bits",
                    ..
                })
            ));
        }

        let mut tampered = certificate.clone();
        tampered.tamper_first_entry_ordinal_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchGuardCompositionError::ReplayMismatch)
        ));
        let mut tampered = certificate.clone();
        tampered.tamper_first_contradiction_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchGuardCompositionError::ReplayMismatch)
        ));
        let mut tampered = certificate;
        tampered.tamper_limits_for_test();
        assert!(matches!(
            tampered.replay(&family, &context),
            Err(ResidualAffineBranchGuardCompositionError::ResourceLimit {
                resource: "guards",
                ..
            })
        ));
    }
}
