//! Matcher-bound affine boundary pullbacks and coefficient numerator gates.
//!
//! This private phase consumes only the authority produced by generated
//! affine `WhenBad` authentication and signed descent.  It has no constructor
//! for an arbitrary relation or affine map.  Every finite boundary hazard is
//! first counted without constructing a polynomial; only after the complete
//! count and all proportional allocations have been admitted is `G_i(t)-v`
//! composed through the selected target's exact integer-system allocation.
//!
//! The table deliberately retains even empty-boundary rows.  They are needed
//! to replay activation-obligation discharge and to prove that an omitted bad
//! clause was false, rather than forgotten.  Structural-locus ordinals are
//! assigned by the later condition-first canonicalizer; this module exposes
//! the exact ordered structural polynomials but never invents a second locus
//! namespace.

use std::fmt;
use std::fmt::Write as _;
use std::mem::{align_of, size_of, size_of_val};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use symbolica::domains::integer::Integer;
use symbolica::poly::PolyVariable;

use crate::generated_residual_affine_when_bad_compilation::GeneratedResidualAffineWhenBadLimits;
use crate::generated_residual_affine_when_bad_descent::{
    GENERATED_RESIDUAL_AFFINE_WHEN_BAD_DESCENT_V1_SCHEMA,
    GeneratedResidualAffineTargetSectorDescentScope, GeneratedResidualAffineWhenBadDescentReady,
    GeneratedResidualAffineWhenBadRhsDescentProof,
};
use crate::parametric_coefficient::{
    ResidualAffineCompositionPlan, ResidualAffineCompositionPlanStats,
};
use crate::when_bad::{WhenBadBoundaryHazardKind, WhenBadCoreError, finite_boundary_hazard_range};
use crate::{
    CoefficientPolynomial, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricPolynomial, ParametricRelation,
    ResidualAffineBranchSystemCertificate, ResidualAffineIntegerSystemCertificate,
    ResidualUnitAffineCompositionError, ResidualUnitAffineCompositionPlanLimits,
    ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffinePolynomialCompositionStats,
};

/// Stable schema for the matcher-bound pullback/gate transcript.
pub(crate) const GENERATED_RESIDUAL_AFFINE_WHEN_BAD_PULLBACK_GATE_V1_SCHEMA: &str =
    "rustred-generated-residual-affine-when-bad-pullback-gate-v1";

/// Child-local projection of all outer budgets consumed by this phase.
///
/// Fields are crate-visible so the outer transaction can subtract work from
/// preceding authentication, descent, and condition phases before invoking
/// this compiler.  No nested allowance is reset per RHS or per pullback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineWhenBadPullbackGateLimits {
    pub(crate) composition_plan: ResidualUnitAffineCompositionPlanLimits,
    pub(crate) polynomial_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) max_boundary_values_per_rhs: usize,
    pub(crate) max_boundary_values: usize,
    pub(crate) max_pullback_compositions: usize,
    pub(crate) max_leak_witnesses: usize,
    pub(crate) max_total_source_terms: usize,
    pub(crate) max_total_source_exponent_entries: usize,
    pub(crate) max_total_source_integer_bits: usize,
    pub(crate) max_total_expanded_contributions: usize,
    pub(crate) max_total_output_term_bound: usize,
    pub(crate) max_total_output_terms: usize,
    pub(crate) max_total_output_exponent_entry_bound: usize,
    pub(crate) max_total_output_exponent_entries: usize,
    pub(crate) max_total_power_calls: usize,
    pub(crate) max_total_native_power_heap_pairs: usize,
    pub(crate) max_total_multiplication_term_pairs: usize,
    pub(crate) max_total_addition_term_visits: usize,
    pub(crate) max_total_native_integer_bit_work: usize,
    pub(crate) max_total_integer_bit_work: usize,
    pub(crate) max_retained_polynomial_terms: usize,
    pub(crate) max_retained_polynomial_exponent_entries: usize,
    pub(crate) max_retained_polynomial_integer_bits: usize,
    pub(crate) max_retained_polynomial_display_bytes: usize,
    pub(crate) max_retained_bytes: usize,
    pub(crate) max_payload_comparison_units: usize,
    pub(crate) max_payload_comparison_bytes: usize,
    pub(crate) max_payload_comparison_integer_bits: usize,
}

impl GeneratedResidualAffineWhenBadPullbackGateLimits {
    pub(crate) const fn from_outer(outer: GeneratedResidualAffineWhenBadLimits) -> Self {
        Self {
            composition_plan: outer.composition_plan,
            polynomial_composition: outer.polynomial_composition,
            max_boundary_values_per_rhs: outer.max_boundary_values_per_rhs,
            max_boundary_values: outer.max_boundary_values,
            max_pullback_compositions: outer.max_pullback_compositions,
            max_leak_witnesses: outer.max_leak_witnesses,
            max_total_source_terms: outer.max_total_source_terms,
            max_total_source_exponent_entries: outer.max_total_source_exponent_entries,
            max_total_source_integer_bits: outer.max_total_source_integer_bits,
            max_total_expanded_contributions: outer.max_total_expanded_contributions,
            max_total_output_term_bound: outer.max_total_output_term_bound,
            max_total_output_terms: outer.max_total_output_terms,
            max_total_output_exponent_entry_bound: outer.max_total_output_exponent_entry_bound,
            max_total_output_exponent_entries: outer.max_total_output_exponent_entries,
            max_total_power_calls: outer.max_total_power_calls,
            max_total_native_power_heap_pairs: outer.max_total_native_power_heap_pairs,
            max_total_multiplication_term_pairs: outer.max_total_multiplication_term_pairs,
            max_total_addition_term_visits: outer.max_total_addition_term_visits,
            max_total_native_integer_bit_work: outer.max_total_native_integer_bit_work,
            max_total_integer_bit_work: outer.max_total_integer_bit_work,
            max_retained_polynomial_terms: outer.max_retained_polynomial_terms,
            max_retained_polynomial_exponent_entries: outer
                .max_retained_polynomial_exponent_entries,
            max_retained_polynomial_integer_bits: outer.max_retained_polynomial_integer_bits,
            max_retained_polynomial_display_bytes: outer.max_retained_polynomial_display_bytes,
            max_retained_bytes: outer.max_retained_bytes,
            max_payload_comparison_units: outer.max_payload_comparison_units,
            max_payload_comparison_bytes: outer.max_payload_comparison_bytes,
            max_payload_comparison_integer_bits: outer.max_payload_comparison_integer_bits,
        }
    }
}

impl Default for GeneratedResidualAffineWhenBadPullbackGateLimits {
    fn default() -> Self {
        Self::from_outer(GeneratedResidualAffineWhenBadLimits::default())
    }
}

/// Exact aggregate work and retained-payload census for the complete table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineWhenBadPullbackGateStats {
    rhs_terms: usize,
    ambient_arity: usize,
    activation_obligations: usize,
    activation_obligation_boundary_values: usize,
    boundary_values: usize,
    pullback_compositions: usize,
    leak_witnesses: usize,
    plan_variables: usize,
    plan_full_images: usize,
    plan_geometry_entries_inspected: usize,
    plan_geometry_entries_retained: usize,
    plan_support_entries_retained: usize,
    plan_total_image_terms: usize,
    plan_total_image_exponent_entries: usize,
    plan_largest_image_integer_bits: usize,
    plan_total_image_integer_bits: usize,
    total_source_terms: usize,
    total_source_exponent_entries: usize,
    total_source_integer_bits: usize,
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
    empty_boundaries: usize,
    whole_target_boundaries: usize,
    free_index_boundaries: usize,
    coefficient_field_numerator_gates: usize,
    free_index_numerator_gates: usize,
    universal_coefficient_nonzero_leaks: usize,
    retained_polynomial_terms: usize,
    retained_polynomial_exponent_entries: usize,
    retained_polynomial_integer_bits: usize,
    retained_polynomial_display_bytes: usize,
    retained_bytes: usize,
    payload_comparison_units: usize,
    payload_comparison_bytes: usize,
    payload_comparison_integer_bits: usize,
}

macro_rules! pullback_gate_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl GeneratedResidualAffineWhenBadPullbackGateStats {
    pullback_gate_stats_getters!(
        rhs_terms,
        ambient_arity,
        activation_obligations,
        activation_obligation_boundary_values,
        boundary_values,
        pullback_compositions,
        leak_witnesses,
        plan_variables,
        plan_full_images,
        plan_geometry_entries_inspected,
        plan_geometry_entries_retained,
        plan_support_entries_retained,
        plan_total_image_terms,
        plan_total_image_exponent_entries,
        plan_largest_image_integer_bits,
        plan_total_image_integer_bits,
        total_source_terms,
        total_source_exponent_entries,
        total_source_integer_bits,
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
        empty_boundaries,
        whole_target_boundaries,
        free_index_boundaries,
        coefficient_field_numerator_gates,
        free_index_numerator_gates,
        universal_coefficient_nonzero_leaks,
        retained_polynomial_terms,
        retained_polynomial_exponent_entries,
        retained_polynomial_integer_bits,
        retained_polynomial_display_bytes,
        retained_bytes,
        payload_comparison_units,
        payload_comparison_bytes,
        payload_comparison_integer_bits,
    );
}

/// Behavior of the exact pulled-back boundary polynomial on the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineBoundaryPullbackClass {
    EmptyBoundary,
    WholeTarget,
    FreeIndexDependent,
}

/// Behavior of the exact normalized RHS numerator on the target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AffineWhenBadNumeratorGateClass {
    CoefficientFieldNonzero,
    FreeIndexNonzero,
}

/// Redacted event metadata safe to surface from a later public certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AffineBoundaryPullbackView {
    ordinal: usize,
    rhs_ordinal: usize,
    hazard_class: WhenBadBoundaryHazardKind,
    pullback_class: AffineBoundaryPullbackClass,
    numerator_gate_class: AffineWhenBadNumeratorGateClass,
}

impl AffineBoundaryPullbackView {
    pub(crate) const fn ordinal(self) -> usize {
        self.ordinal
    }

    pub(crate) const fn rhs_ordinal(self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn hazard_class(self) -> WhenBadBoundaryHazardKind {
        self.hazard_class
    }

    pub(crate) const fn pullback_class(self) -> AffineBoundaryPullbackClass {
        self.pullback_class
    }

    pub(crate) const fn numerator_gate_class(self) -> AffineWhenBadNumeratorGateClass {
        self.numerator_gate_class
    }
}

/// Exact private normalized numerator gate.  It is copied from the already
/// mapped/recentered coefficient and is never specialized at the boundary or
/// composed through the target plan a second time.
#[derive(PartialEq, Eq)]
pub(crate) enum AffineWhenBadNumeratorGatePayload {
    CoefficientFieldNonzero(ParametricPolynomial),
    FreeIndexNonzero(ParametricPolynomial),
}

impl AffineWhenBadNumeratorGatePayload {
    pub(crate) const fn class(&self) -> AffineWhenBadNumeratorGateClass {
        match self {
            Self::CoefficientFieldNonzero(_) => {
                AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero
            }
            Self::FreeIndexNonzero(_) => AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
        }
    }

    pub(crate) const fn polynomial(&self) -> &ParametricPolynomial {
        match self {
            Self::CoefficientFieldNonzero(polynomial) | Self::FreeIndexNonzero(polynomial) => {
                polynomial
            }
        }
    }
}

impl fmt::Debug for AffineWhenBadNumeratorGatePayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AffineWhenBadNumeratorGatePayload")
            .field("class", &self.class())
            .field("polynomial", &"<redacted>")
            .finish()
    }
}

/// Exact private binding to one conditional target-sector obligation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineActivationObligationLocator {
    target_witness_ordinal: usize,
    obligation_ordinal_within_witness: usize,
}

impl GeneratedResidualAffineActivationObligationLocator {
    pub(crate) const fn target_witness_ordinal(self) -> usize {
        self.target_witness_ordinal
    }

    pub(crate) const fn obligation_ordinal_within_witness(self) -> usize {
        self.obligation_ordinal_within_witness
    }
}

impl fmt::Debug for GeneratedResidualAffineActivationObligationLocator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GeneratedResidualAffineActivationObligationLocator(<redacted>)")
    }
}

/// Exact source locator for one retained boundary event.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineBoundarySourceProvenance {
    rhs_ordinal: usize,
    descent_witness_ordinal: usize,
    target_sector_descent: bool,
    activation_obligation: Option<GeneratedResidualAffineActivationObligationLocator>,
}

impl GeneratedResidualAffineBoundarySourceProvenance {
    pub(crate) const fn rhs_ordinal(self) -> usize {
        self.rhs_ordinal
    }

    pub(crate) const fn descent_witness_ordinal(self) -> usize {
        self.descent_witness_ordinal
    }

    pub(crate) const fn is_target_sector_descent(self) -> bool {
        self.target_sector_descent
    }

    pub(crate) const fn activation_obligation(
        self,
    ) -> Option<GeneratedResidualAffineActivationObligationLocator> {
        self.activation_obligation
    }
}

impl fmt::Debug for GeneratedResidualAffineBoundarySourceProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineBoundarySourceProvenance")
            .field("rhs_ordinal", &self.rhs_ordinal)
            .field(
                "descent_route",
                &if self.target_sector_descent {
                    "target-sector"
                } else {
                    "same-sector"
                },
            )
            .field(
                "activation_obligation",
                &self.activation_obligation.map(|_| "<redacted>"),
            )
            .finish()
    }
}

/// One exact ordered `B(t)=G_i(t)-v`, `N_s(t)` pair.
#[derive(PartialEq, Eq)]
pub(crate) struct GeneratedResidualAffineWhenBadPullbackGateEvent {
    ordinal: usize,
    provenance: GeneratedResidualAffineBoundarySourceProvenance,
    hazard_class: WhenBadBoundaryHazardKind,
    ambient_coordinate: usize,
    boundary_value: i64,
    pullback: ParametricPolynomial,
    pullback_class: AffineBoundaryPullbackClass,
    numerator_gate: AffineWhenBadNumeratorGatePayload,
    composition_stats: ResidualUnitAffinePolynomialCompositionStats,
}

impl GeneratedResidualAffineWhenBadPullbackGateEvent {
    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub(crate) const fn provenance(&self) -> GeneratedResidualAffineBoundarySourceProvenance {
        self.provenance
    }

    pub(crate) const fn hazard_class(&self) -> WhenBadBoundaryHazardKind {
        self.hazard_class
    }

    pub(crate) const fn ambient_coordinate(&self) -> usize {
        self.ambient_coordinate
    }

    pub(crate) const fn boundary_value(&self) -> i64 {
        self.boundary_value
    }

    pub(crate) const fn pullback(&self) -> &ParametricPolynomial {
        &self.pullback
    }

    pub(crate) const fn pullback_class(&self) -> AffineBoundaryPullbackClass {
        self.pullback_class
    }

    pub(crate) const fn numerator_gate(&self) -> &AffineWhenBadNumeratorGatePayload {
        &self.numerator_gate
    }

    pub(crate) const fn composition_stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.composition_stats
    }

    /// Ordered structural-polynomial input for the later condition-first
    /// locus canonicalizer.  Empty boundaries still expose their pullback for
    /// replay, but callers omit it from the semantic bad formula.
    pub(crate) const fn structural_polynomials(
        &self,
    ) -> (&ParametricPolynomial, &ParametricPolynomial) {
        (&self.pullback, self.numerator_gate.polynomial())
    }

    pub(crate) const fn view(&self) -> AffineBoundaryPullbackView {
        AffineBoundaryPullbackView {
            ordinal: self.ordinal,
            rhs_ordinal: self.provenance.rhs_ordinal,
            hazard_class: self.hazard_class,
            pullback_class: self.pullback_class,
            numerator_gate_class: self.numerator_gate.class(),
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadPullbackGateEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadPullbackGateEvent")
            .field("view", &self.view())
            .field("provenance", &self.provenance)
            .field("ambient_coordinate", &"<redacted>")
            .field("boundary_value", &"<redacted>")
            .field("pullback", &"<redacted>")
            .field("numerator_gate", &self.numerator_gate)
            .field("composition_stats", &self.composition_stats)
            .finish()
    }
}

/// Complete private table bound to the exact matcher, relation, target branch,
/// integer-system allocation, and source-neutral composition plan.
pub(crate) struct GeneratedResidualAffineWhenBadPullbackGateCertificate {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    source_case_ordinal: usize,
    source_group_ordinal: usize,
    pivot_ordinal: usize,
    target_case_ordinal: usize,
    matcher: Arc<crate::GeneratedResidualAffinePivotTargetMatchingCertificate>,
    relation: Arc<ParametricRelation>,
    target_branch: Arc<ResidualAffineBranchSystemCertificate>,
    integer_system: Arc<ResidualAffineIntegerSystemCertificate>,
    composition_plan: Arc<ResidualAffineCompositionPlan>,
    events: Vec<GeneratedResidualAffineWhenBadPullbackGateEvent>,
    universal_coefficient_nonzero_leak_ordinal: Option<usize>,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
    stats: GeneratedResidualAffineWhenBadPullbackGateStats,
}

impl GeneratedResidualAffineWhenBadPullbackGateCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn events(&self) -> &[GeneratedResidualAffineWhenBadPullbackGateEvent] {
        &self.events
    }

    pub(crate) fn event_view(&self, ordinal: usize) -> Option<AffineBoundaryPullbackView> {
        self.events.get(ordinal).map(|event| event.view())
    }

    pub(crate) const fn universal_coefficient_nonzero_leak_ordinal(&self) -> Option<usize> {
        self.universal_coefficient_nonzero_leak_ordinal
    }

    pub(crate) const fn limits(&self) -> GeneratedResidualAffineWhenBadPullbackGateLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> GeneratedResidualAffineWhenBadPullbackGateStats {
        self.stats
    }

    pub(crate) const fn composition_plan(&self) -> &Arc<ResidualAffineCompositionPlan> {
        &self.composition_plan
    }

    pub(crate) const fn integer_system(&self) -> &Arc<ResidualAffineIntegerSystemCertificate> {
        &self.integer_system
    }

    pub(crate) fn payload_eq_checked(
        &self,
        other: &Self,
    ) -> Result<bool, GeneratedResidualAffineWhenBadPullbackGateError> {
        if self.limits != other.limits {
            return Ok(false);
        }
        preflight_payload_comparison(self, other)?;
        Ok(self.schema == other.schema
            && self.context_fingerprint == other.context_fingerprint
            && self.source_case_ordinal == other.source_case_ordinal
            && self.source_group_ordinal == other.source_group_ordinal
            && self.pivot_ordinal == other.pivot_ordinal
            && self.target_case_ordinal == other.target_case_ordinal
            && Arc::ptr_eq(&self.matcher, &other.matcher)
            && Arc::ptr_eq(&self.relation, &other.relation)
            && Arc::ptr_eq(&self.target_branch, &other.target_branch)
            && Arc::ptr_eq(&self.integer_system, &other.integer_system)
            && self.composition_plan.limits() == other.composition_plan.limits()
            && self.composition_plan.stats() == other.composition_plan.stats()
            && Arc::ptr_eq(
                self.composition_plan.certificate(),
                other.composition_plan.certificate(),
            )
            && self.events == other.events
            && self.universal_coefficient_nonzero_leak_ordinal
                == other.universal_coefficient_nonzero_leak_ordinal
            && self.limits == other.limits
            && self.stats == other.stats)
    }

    pub(crate) fn replay(
        &self,
        context: &ParametricCoefficientContext,
        ready: &GeneratedResidualAffineWhenBadDescentReady,
    ) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
        validate_ready_binding(context, ready, Some(self))?;
        let replayed = compile_generated_residual_affine_when_bad_pullback_gate_table(
            context,
            ready,
            self.limits,
        )?;
        let replayed_certificate = replayed.certificate();
        if replayed.universal_coefficient_nonzero_leak_ordinal()
            != self.universal_coefficient_nonzero_leak_ordinal
            || !self.payload_eq_checked(replayed_certificate)?
        {
            return Err(GeneratedResidualAffineWhenBadPullbackGateError::ReplayMismatch);
        }
        Ok(())
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadPullbackGateCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadPullbackGateCertificate")
            .field("schema", &self.schema)
            .field("context_fingerprint", &"<redacted>")
            .field("source_case_ordinal", &self.source_case_ordinal)
            .field("source_group_ordinal", &self.source_group_ordinal)
            .field("pivot_ordinal", &self.pivot_ordinal)
            .field("target_case_ordinal", &self.target_case_ordinal)
            .field("private_authority", &"<redacted>")
            .field("events", &self.events.len())
            .field(
                "universal_coefficient_nonzero_leak_ordinal",
                &self.universal_coefficient_nonzero_leak_ordinal,
            )
            .field("limits", &self.limits)
            .field("stats", &self.stats)
            .finish()
    }
}

/// Typed evidence that a whole-target boundary has a coefficient-field
/// nonzero numerator.  The complete table is retained so replay never
/// authenticates only a short-circuited event prefix.
pub(crate) struct GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad {
    certificate: GeneratedResidualAffineWhenBadPullbackGateCertificate,
    universal_coefficient_nonzero_leak_ordinal: usize,
}

impl GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad {
    pub(crate) const fn certificate(
        &self,
    ) -> &GeneratedResidualAffineWhenBadPullbackGateCertificate {
        &self.certificate
    }

    pub(crate) const fn universal_coefficient_nonzero_leak_ordinal(&self) -> usize {
        self.universal_coefficient_nonzero_leak_ordinal
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad")
            .field(
                "universal_coefficient_nonzero_leak_ordinal",
                &self.universal_coefficient_nonzero_leak_ordinal,
            )
            .field("certificate", &self.certificate)
            .finish()
    }
}

/// Complete table outcome.  `IdenticallyBad` is semantic, not a hard error.
pub(crate) enum GeneratedResidualAffineWhenBadPullbackGateCompilation {
    Ready(GeneratedResidualAffineWhenBadPullbackGateCertificate),
    IdenticallyBad(GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad),
}

impl GeneratedResidualAffineWhenBadPullbackGateCompilation {
    pub(crate) const fn certificate(
        &self,
    ) -> &GeneratedResidualAffineWhenBadPullbackGateCertificate {
        match self {
            Self::Ready(certificate) => certificate,
            Self::IdenticallyBad(outcome) => outcome.certificate(),
        }
    }

    pub(crate) const fn universal_coefficient_nonzero_leak_ordinal(&self) -> Option<usize> {
        match self {
            Self::Ready(_) => None,
            Self::IdenticallyBad(outcome) => {
                Some(outcome.universal_coefficient_nonzero_leak_ordinal())
            }
        }
    }

    pub(crate) fn into_certificate(self) -> GeneratedResidualAffineWhenBadPullbackGateCertificate {
        match self {
            Self::Ready(certificate) => certificate,
            Self::IdenticallyBad(outcome) => outcome.certificate,
        }
    }
}

impl fmt::Debug for GeneratedResidualAffineWhenBadPullbackGateCompilation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready(certificate) => formatter.debug_tuple("Ready").field(certificate).finish(),
            Self::IdenticallyBad(outcome) => formatter
                .debug_tuple("IdenticallyBad")
                .field(outcome)
                .finish(),
        }
    }
}

/// Hard authentication, resource, allocation, composition, or replay error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum GeneratedResidualAffineWhenBadPullbackGateError {
    SchemaMismatch,
    WrongContext,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    MissingTargetIntegerSystem,
    CompositionPlanIntegerSystemAllocationMismatch,
    CompositionPlanStatsMismatch {
        resource: &'static str,
    },
    ReadyAuthorityMismatch,
    PrivateRhsCountMismatch {
        authenticated: usize,
        observed: usize,
    },
    DescentProofMismatch {
        rhs_ordinal: usize,
    },
    ActivationObligationMismatch {
        rhs_ordinal: usize,
    },
    NonfreeNumeratorSupport {
        rhs_ordinal: usize,
    },
    ZeroNumeratorGate {
        rhs_ordinal: usize,
    },
    ReplayMismatch,
    RetainedByteEnvelopeExceeded {
        observed: usize,
        admitted: usize,
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
        requested: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
    ParametricCoefficient(ParametricCoefficientError),
    Composition(ResidualUnitAffineCompositionError),
    Core(WhenBadCoreError),
}

impl fmt::Display for GeneratedResidualAffineWhenBadPullbackGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => formatter.write_str("generated affine pullback/gate schema mismatch"),
            Self::WrongContext => formatter.write_str("generated affine pullback/gate table belongs to another K(n) context"),
            Self::WrongArity { expected, actual } => write!(formatter, "generated affine pullback/gate table expected arity {expected}, got {actual}"),
            Self::MissingTargetIntegerSystem => formatter.write_str("generated affine pullback/gate target has no authenticated integer system"),
            Self::CompositionPlanIntegerSystemAllocationMismatch => formatter.write_str("generated affine pullback/gate plan did not retain the selected target integer-system allocation"),
            Self::CompositionPlanStatsMismatch { resource } => write!(formatter, "generated affine pullback/gate composition-plan statistics are inconsistent for {resource}"),
            Self::ReadyAuthorityMismatch => formatter.write_str("generated affine pullback/gate table is bound to another authenticated descent authority"),
            Self::PrivateRhsCountMismatch { authenticated, observed } => write!(formatter, "generated affine pullback/gate authenticated {authenticated} RHS terms, observed {observed}"),
            Self::DescentProofMismatch { rhs_ordinal } => write!(formatter, "generated affine pullback/gate descent proof mismatch at RHS ordinal {rhs_ordinal}"),
            Self::ActivationObligationMismatch { rhs_ordinal } => write!(formatter, "generated affine pullback/gate activation-obligation mismatch at RHS ordinal {rhs_ordinal}"),
            Self::NonfreeNumeratorSupport { rhs_ordinal } => write!(formatter, "generated affine pullback/gate numerator at RHS ordinal {rhs_ordinal} retains nonfree index support"),
            Self::ZeroNumeratorGate { rhs_ordinal } => write!(formatter, "generated affine pullback/gate numerator at RHS ordinal {rhs_ordinal} is zero"),
            Self::ReplayMismatch => formatter.write_str("generated affine pullback/gate table did not replay exactly"),
            Self::RetainedByteEnvelopeExceeded { observed, admitted } => write!(formatter, "generated affine pullback/gate retained {observed} bytes after admitting {admitted}"),
            Self::ResourceLimit { resource, requested, limit } => write!(formatter, "{resource} requested {requested}, configured limit is {limit}"),
            Self::ResourceCountOverflow { resource } => write!(formatter, "{resource} count overflowed usize"),
            Self::AllocationFailure { resource, requested } => write!(formatter, "{resource} allocation of {requested} entries failed after bounded preflight"),
            Self::SymbolicaPanic { stage } => write!(formatter, "Symbolica panicked during generated affine pullback/gate {stage}"),
            Self::ParametricCoefficient(error) => error.fmt(formatter),
            Self::Composition(error) => error.fmt(formatter),
            Self::Core(error) => write_when_bad_core_error(formatter, error),
        }
    }
}

impl std::error::Error for GeneratedResidualAffineWhenBadPullbackGateError {}

fn write_when_bad_core_error(
    formatter: &mut fmt::Formatter<'_>,
    error: &WhenBadCoreError,
) -> fmt::Result {
    match error {
        WhenBadCoreError::WrongArity { expected, actual } => write!(
            formatter,
            "generated affine pullback/gate expected arity {expected}, got {actual}"
        ),
        WhenBadCoreError::BoundaryArithmeticOverflow { coordinate } => write!(
            formatter,
            "generated affine pullback/gate boundary arithmetic overflow at coordinate {coordinate}"
        ),
        WhenBadCoreError::DescentArithmeticOverflow => {
            formatter.write_str("generated affine pullback/gate descent arithmetic overflow")
        }
        WhenBadCoreError::RetainedCapacityEnvelopeExceeded {
            resource,
            observed_bytes,
            admitted_bytes,
        } => write!(
            formatter,
            "{resource} retained {observed_bytes} bytes after admitting {admitted_bytes}"
        ),
        WhenBadCoreError::ResourceCountOverflow { resource } => {
            write!(formatter, "{resource} count overflowed usize")
        }
        WhenBadCoreError::AllocationFailure {
            resource,
            requested,
        } => write!(
            formatter,
            "{resource} allocation of {requested} entries failed after bounded preflight"
        ),
        WhenBadCoreError::ParametricRelation(error) => write!(formatter, "{error}"),
    }
}

impl From<ParametricCoefficientError> for GeneratedResidualAffineWhenBadPullbackGateError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

impl From<ResidualUnitAffineCompositionError> for GeneratedResidualAffineWhenBadPullbackGateError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<WhenBadCoreError> for GeneratedResidualAffineWhenBadPullbackGateError {
    fn from(value: WhenBadCoreError) -> Self {
        Self::Core(value)
    }
}

/// Compile every finite affine boundary pullback after complete hazard census.
pub(crate) fn compile_generated_residual_affine_when_bad_pullback_gate_table(
    context: &ParametricCoefficientContext,
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<
    GeneratedResidualAffineWhenBadPullbackGateCompilation,
    GeneratedResidualAffineWhenBadPullbackGateError,
> {
    catch_unwind(AssertUnwindSafe(|| {
        compile_generated_residual_affine_when_bad_pullback_gate_table_inner(context, ready, limits)
    }))
    .map_err(
        |_| GeneratedResidualAffineWhenBadPullbackGateError::SymbolicaPanic {
            stage: "compilation",
        },
    )?
}

fn compile_generated_residual_affine_when_bad_pullback_gate_table_inner(
    context: &ParametricCoefficientContext,
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<
    GeneratedResidualAffineWhenBadPullbackGateCompilation,
    GeneratedResidualAffineWhenBadPullbackGateError,
> {
    validate_ready_binding(context, ready, None)?;
    let input = ready.input();
    let relation = input.relation();
    let arity = context.index_count();
    let rhs_count = ready.binding().rhs_terms();

    let census = census_hazards(ready, limits)?;
    if census.rhs_terms != rhs_count {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::PrivateRhsCountMismatch {
                authenticated: rhs_count,
                observed: census.rhs_terms,
            },
        );
    }

    // The complete proportional event allocation is admitted before the
    // target plan or any source/boundary polynomial is constructed.
    let event_capacity_envelope = capacity_envelope(census.boundary_values)?;
    let pre_plan_retained_envelope = checked_add(
        "generated affine pullback/gate retained bytes",
        size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
        checked_mul(
            "generated affine pullback/gate retained bytes",
            event_capacity_envelope,
            size_of::<GeneratedResidualAffineWhenBadPullbackGateEvent>(),
        )?,
    )?;
    check_limit(
        "generated affine pullback/gate retained bytes",
        pre_plan_retained_envelope,
        limits.max_retained_bytes,
    )?;
    let mut events = Vec::new();
    events
        .try_reserve_exact(census.boundary_values)
        .map_err(
            |_| GeneratedResidualAffineWhenBadPullbackGateError::AllocationFailure {
                resource: "generated affine pullback/gate events",
                requested: census.boundary_values,
            },
        )?;
    if events.capacity() > event_capacity_envelope {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::RetainedByteEnvelopeExceeded {
                observed: events.capacity(),
                admitted: event_capacity_envelope,
            },
        );
    }

    let integer_system = input
        .target_branch()
        .integer_system_arc()
        .ok_or(GeneratedResidualAffineWhenBadPullbackGateError::MissingTargetIntegerSystem)?
        .clone();
    // Admit the complete outer/core Arc allocations and every durable core
    // vector before the child compiler can allocate its compact geometry or
    // enter native polynomial construction.  The authenticated ambient map
    // supplies these counts allocation-free.
    let preflight_plan = preflight_composition_plan_retained_shape(
        context,
        &integer_system,
        limits.composition_plan,
    )?;
    let preflight_plan_retained_envelope =
        composition_plan_retained_byte_census(preflight_plan)?.total()?;
    let mut retained_envelope = admit_retained_envelope_before_plan(
        pre_plan_retained_envelope,
        preflight_plan_retained_envelope,
        context.fingerprint().len(),
        limits.max_retained_bytes,
    )?;
    let composition_plan = Arc::new(
        context.compile_residual_affine_composition_plan_from_integer_system(
            integer_system.clone(),
            limits.composition_plan,
        )?,
    );
    if !Arc::ptr_eq(composition_plan.certificate(), &integer_system) {
        return Err(GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanIntegerSystemAllocationMismatch);
    }
    let plan_stats = composition_plan.stats();
    let actual_plan_shape = CompositionPlanRetainedShape::from_stats(plan_stats, arity)?;
    if actual_plan_shape != preflight_plan.shape {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                resource: "authenticated preflight versus retained child plan",
            },
        );
    }
    let actual_plan_retained_envelope = composition_plan.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
            resource: "generated affine pullback/gate observed composition-plan retained bytes",
        },
    )?;
    if actual_plan_retained_envelope > preflight_plan_retained_envelope {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::RetainedByteEnvelopeExceeded {
                observed: actual_plan_retained_envelope,
                admitted: preflight_plan_retained_envelope,
            },
        );
    }

    let mut stats = GeneratedResidualAffineWhenBadPullbackGateStats {
        rhs_terms: rhs_count,
        ambient_arity: arity,
        activation_obligations: census.activation_obligations,
        activation_obligation_boundary_values: census.activation_obligation_boundary_values,
        boundary_values: census.boundary_values,
        pullback_compositions: census.boundary_values,
        leak_witnesses: census.boundary_values,
        plan_variables: plan_stats.variables(),
        plan_full_images: plan_stats.full_images(),
        plan_geometry_entries_inspected: plan_stats.geometry_entries_inspected(),
        plan_geometry_entries_retained: plan_stats.geometry_entries_retained(),
        plan_support_entries_retained: plan_stats.support_entries_retained(),
        plan_total_image_terms: plan_stats.total_image_terms(),
        plan_total_image_exponent_entries: plan_stats.total_image_exponent_entries(),
        plan_largest_image_integer_bits: plan_stats.largest_image_integer_bits(),
        plan_total_image_integer_bits: plan_stats.total_image_integer_bits(),
        ..GeneratedResidualAffineWhenBadPullbackGateStats::default()
    };
    let mut universal_coefficient_nonzero_leak_ordinal = None;
    let mut rhs_ordinal = 0usize;
    for (shift, coefficient) in relation.terms() {
        if shift.values().iter().all(|value| *value == 0) {
            continue;
        }
        let proof = ready.private_rhs_proofs().get(rhs_ordinal).copied().ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch { rhs_ordinal },
        )?;
        let gate_shape = coefficient_numerator_shape(coefficient)?;
        if gate_shape.terms == 0 {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ZeroNumeratorGate { rhs_ordinal },
            );
        }
        validate_numerator_free_support(
            context,
            coefficient,
            integer_system.free_positions(),
            rhs_ordinal,
        )?;
        let gate_depends_on_indices =
            coefficient_numerator_depends_on_indices(context, coefficient);

        for (coordinate, (&active, &delta)) in ready
            .binding()
            .sector()
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let Some(hazard) = finite_boundary_hazard_range(active, delta, coordinate)? else {
                continue;
            };
            let mut boundary_value = hazard.first();
            loop {
                let ordinal = events.len();
                let activation_obligation = activation_obligation_for_boundary(
                    ready,
                    proof,
                    rhs_ordinal,
                    coordinate,
                    boundary_value,
                    hazard.kind(),
                )?;
                let provenance = GeneratedResidualAffineBoundarySourceProvenance {
                    rhs_ordinal,
                    descent_witness_ordinal: proof.witness_ordinal(),
                    target_sector_descent: proof.is_target_sector(),
                    activation_obligation,
                };

                let expected_source_shape = boundary_source_shape(context, boundary_value)?;
                precharge_source_shapes(&mut stats, expected_source_shape, gate_shape, limits)?;
                let source = boundary_source_polynomial(
                    context,
                    coordinate,
                    boundary_value,
                    limits.polynomial_composition,
                )?;
                let source_shape = polynomial_shape(&source)?;
                if source_shape != expected_source_shape {
                    return Err(GeneratedResidualAffineWhenBadPullbackGateError::ReplayMismatch);
                }
                let effective_limits = remaining_composition_limits(limits, &stats)?;
                let preflight = context.preflight_polynomial_on_residual_affine_composition_plan(
                    &source,
                    &composition_plan,
                    effective_limits,
                )?;
                if preflight.source_terms() != expected_source_shape.terms
                    || preflight.source_exponent_entries() != expected_source_shape.exponent_entries
                {
                    return Err(GeneratedResidualAffineWhenBadPullbackGateError::ReplayMismatch);
                }
                // The compositor has one combined integer-work bound.  The
                // native-only aggregate is independently admitted here before
                // entering its evaluator.
                check_limit(
                    "generated affine pullback/gate total native integer-bit work",
                    checked_add(
                        "generated affine pullback/gate total native integer-bit work",
                        stats.total_native_integer_bit_work,
                        preflight.native_integer_bit_work_bound(),
                    )?,
                    limits.max_total_native_integer_bit_work,
                )?;
                let admitted_event_bytes = preflight_retained_pullback_envelope(
                    &mut retained_envelope,
                    &stats,
                    preflight,
                    gate_shape,
                    plan_stats.variables(),
                    limits,
                )?;
                let composition = context.compose_polynomial_on_residual_affine_composition_plan(
                    &source,
                    &composition_plan,
                    effective_limits,
                )?;
                let (pullback, composition_stats) = composition.into_parts();
                if !same_preflight_work(preflight, composition_stats) {
                    return Err(GeneratedResidualAffineWhenBadPullbackGateError::ReplayMismatch);
                }
                aggregate_composition_stats(&mut stats, composition_stats, limits)?;
                let _pullback_shape = retain_polynomial_shape(&pullback, &mut stats, limits)?;
                charge_retained_polynomial_display(&pullback, &mut stats, limits)?;
                validate_polynomial_actual_byte_envelope(&pullback, admitted_event_bytes.pullback)?;

                let pullback_depends = context.polynomial_depends_on_indices_with_limits(
                    &pullback,
                    limits.polynomial_composition.exact_algebra,
                )?;
                let pullback_class = if pullback.is_zero() {
                    stats.whole_target_boundaries = checked_add(
                        "generated affine pullback/gate whole-target boundaries",
                        stats.whole_target_boundaries,
                        1,
                    )?;
                    AffineBoundaryPullbackClass::WholeTarget
                } else if pullback_depends {
                    stats.free_index_boundaries = checked_add(
                        "generated affine pullback/gate free-index boundaries",
                        stats.free_index_boundaries,
                        1,
                    )?;
                    AffineBoundaryPullbackClass::FreeIndexDependent
                } else {
                    stats.empty_boundaries = checked_add(
                        "generated affine pullback/gate empty boundaries",
                        stats.empty_boundaries,
                        1,
                    )?;
                    AffineBoundaryPullbackClass::EmptyBoundary
                };

                // All shape/byte limits have been charged before this exact
                // fallible sparse-payload copy.
                let gate_polynomial = coefficient
                    .try_copy_prevalidated_numerator_condition()
                    .map_err(|resource| {
                        GeneratedResidualAffineWhenBadPullbackGateError::AllocationFailure {
                            resource,
                            requested: gate_shape.terms,
                        }
                    })?;
                validate_polynomial_actual_byte_envelope(
                    &gate_polynomial,
                    admitted_event_bytes.numerator_gate,
                )?;
                retain_polynomial_shape_precomputed(gate_shape, &mut stats, limits)?;
                charge_retained_polynomial_display(&gate_polynomial, &mut stats, limits)?;
                let numerator_gate = if gate_depends_on_indices {
                    stats.free_index_numerator_gates = checked_add(
                        "generated affine pullback/gate free-index numerator gates",
                        stats.free_index_numerator_gates,
                        1,
                    )?;
                    AffineWhenBadNumeratorGatePayload::FreeIndexNonzero(gate_polynomial)
                } else {
                    stats.coefficient_field_numerator_gates = checked_add(
                        "generated affine pullback/gate coefficient-field numerator gates",
                        stats.coefficient_field_numerator_gates,
                        1,
                    )?;
                    AffineWhenBadNumeratorGatePayload::CoefficientFieldNonzero(gate_polynomial)
                };
                if pullback_class == AffineBoundaryPullbackClass::WholeTarget
                    && numerator_gate.class()
                        == AffineWhenBadNumeratorGateClass::CoefficientFieldNonzero
                {
                    stats.universal_coefficient_nonzero_leaks = checked_add(
                        "generated affine pullback/gate universal coefficient-nonzero leaks",
                        stats.universal_coefficient_nonzero_leaks,
                        1,
                    )?;
                    if universal_coefficient_nonzero_leak_ordinal.is_none() {
                        universal_coefficient_nonzero_leak_ordinal = Some(ordinal);
                    }
                }
                events.push(GeneratedResidualAffineWhenBadPullbackGateEvent {
                    ordinal,
                    provenance,
                    hazard_class: hazard.kind(),
                    ambient_coordinate: coordinate,
                    boundary_value,
                    pullback,
                    pullback_class,
                    numerator_gate,
                    composition_stats,
                });
                if boundary_value == hazard.last() {
                    break;
                }
                boundary_value = boundary_value.checked_add(1).ok_or(
                    GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
                        resource: "generated affine pullback/gate boundary iterator",
                    },
                )?;
            }
        }
        rhs_ordinal = checked_add("generated affine pullback/gate RHS ordinal", rhs_ordinal, 1)?;
    }
    if rhs_ordinal != rhs_count || events.len() != census.boundary_values {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::PrivateRhsCountMismatch {
                authenticated: census.boundary_values,
                observed: events.len(),
            },
        );
    }
    validate_activation_obligation_event_coverage(ready, &events, &census)?;
    stats.retained_bytes = retained_envelope;

    let context_fingerprint: Arc<str> = Arc::from(context.fingerprint());
    let binding = ready.binding();
    let mut certificate = GeneratedResidualAffineWhenBadPullbackGateCertificate {
        schema: GENERATED_RESIDUAL_AFFINE_WHEN_BAD_PULLBACK_GATE_V1_SCHEMA,
        context_fingerprint,
        source_case_ordinal: binding.source_case_ordinal(),
        source_group_ordinal: binding.source_group_ordinal(),
        pivot_ordinal: binding.pivot_ordinal(),
        target_case_ordinal: binding.target_case_ordinal(),
        matcher: input.matcher().clone(),
        relation: relation.clone(),
        target_branch: input.target_branch().clone(),
        integer_system,
        composition_plan,
        events,
        universal_coefficient_nonzero_leak_ordinal,
        limits,
        stats,
    };
    authenticate_payload_comparison_stats(&mut certificate)?;
    let Some(ordinal) = universal_coefficient_nonzero_leak_ordinal else {
        return Ok(GeneratedResidualAffineWhenBadPullbackGateCompilation::Ready(certificate));
    };
    Ok(
        GeneratedResidualAffineWhenBadPullbackGateCompilation::IdenticallyBad(
            GeneratedResidualAffineWhenBadPullbackGateIdenticallyBad {
                certificate,
                universal_coefficient_nonzero_leak_ordinal: ordinal,
            },
        ),
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct HazardCensus {
    rhs_terms: usize,
    boundary_values: usize,
    activation_obligations: usize,
    activation_obligation_boundary_values: usize,
}

fn census_hazards(
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<HazardCensus, GeneratedResidualAffineWhenBadPullbackGateError> {
    let relation = ready.input().relation();
    let mut census = HazardCensus::default();
    for (shift, _) in relation.terms() {
        if shift.values().iter().all(|value| *value == 0) {
            continue;
        }
        let rhs_ordinal = census.rhs_terms;
        let proof = ready.private_rhs_proofs().get(rhs_ordinal).copied().ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch { rhs_ordinal },
        )?;
        validate_descent_proof_binding(ready, proof, rhs_ordinal, shift)?;
        let mut rhs_boundary_values = 0usize;
        for (coordinate, (&active, &delta)) in ready
            .binding()
            .sector()
            .active_bits()
            .iter()
            .zip(shift.values())
            .enumerate()
        {
            let Some(hazard) = finite_boundary_hazard_range(active, delta, coordinate)? else {
                continue;
            };
            rhs_boundary_values = bounded_add(
                "generated affine pullback/gate boundary values per RHS",
                rhs_boundary_values,
                hazard.count(),
                limits.max_boundary_values_per_rhs,
            )?;
        }
        census.boundary_values = bounded_add(
            "generated affine pullback/gate boundary values",
            census.boundary_values,
            rhs_boundary_values,
            limits.max_boundary_values,
        )?;
        check_limit(
            "generated affine pullback/gate pullback compositions",
            census.boundary_values,
            limits.max_pullback_compositions,
        )?;
        check_limit(
            "generated affine pullback/gate leak witnesses",
            census.boundary_values,
            limits.max_leak_witnesses,
        )?;
        census.rhs_terms = checked_add(
            "generated affine pullback/gate RHS terms",
            census.rhs_terms,
            1,
        )?;
    }
    if census.rhs_terms != ready.binding().rhs_terms()
        || census.rhs_terms != ready.private_rhs_proofs().len()
    {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::PrivateRhsCountMismatch {
                authenticated: ready.binding().rhs_terms(),
                observed: census.rhs_terms,
            },
        );
    }

    let rhs = relation
        .terms()
        .iter()
        .filter(|(shift, _)| shift.values().iter().any(|value| *value != 0));
    for (rhs_ordinal, ((shift, _), proof)) in rhs
        .zip(ready.private_rhs_proofs().iter().copied())
        .enumerate()
    {
        if !proof.is_target_sector() {
            continue;
        }
        let witness = ready
            .private_target_sector_transcript()
            .witnesses()
            .get(proof.witness_ordinal())
            .copied()
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch {
                    rhs_ordinal,
                },
            )?;
        let obligations = ready
            .private_target_sector_transcript()
            .symbolic_activation_obligations(witness)
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                    rhs_ordinal,
                },
            )?;
        if witness.scope()
            == GeneratedResidualAffineTargetSectorDescentScope::ApplicableNonzeroTermDomain
            && obligations.is_empty()
        {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                    rhs_ordinal,
                },
            );
        }
        for (obligation_ordinal, obligation) in obligations.iter().copied().enumerate() {
            if obligation.count() == 0
                || obligation.first() > obligation.last()
                || obligation.position() >= ready.binding().sector().arity()
                || ready.binding().sector().active_bits()[obligation.position()]
            {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                        rhs_ordinal,
                    },
                );
            }
            let hazard = finite_boundary_hazard_range(
                false,
                shift.values()[obligation.position()],
                obligation.position(),
            )?
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                    rhs_ordinal,
                },
            )?;
            if hazard.kind() != WhenBadBoundaryHazardKind::InactiveSectorActivation
                || hazard.first() != obligation.first()
                || hazard.last() != obligation.last()
                || hazard.count() != obligation.count()
            {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                        rhs_ordinal,
                    },
                );
            }
            // Reject duplicate obligations before an event could match two
            // provenance owners.
            if obligation_ordinal > 0
                && obligations[obligation_ordinal - 1].position() >= obligation.position()
            {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                        rhs_ordinal,
                    },
                );
            }
            census.activation_obligations = checked_add(
                "generated affine pullback/gate activation obligations",
                census.activation_obligations,
                1,
            )?;
            census.activation_obligation_boundary_values = checked_add(
                "generated affine pullback/gate activation-obligation boundary values",
                census.activation_obligation_boundary_values,
                obligation.count(),
            )?;
        }
    }
    if ready.requires_symbolic_activation_hazard_seal() && census.activation_obligations == 0 {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                rhs_ordinal: 0,
            },
        );
    }
    Ok(census)
}

fn validate_ready_binding(
    context: &ParametricCoefficientContext,
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    retained: Option<&GeneratedResidualAffineWhenBadPullbackGateCertificate>,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    if ready.schema() != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_DESCENT_V1_SCHEMA {
        return Err(GeneratedResidualAffineWhenBadPullbackGateError::SchemaMismatch);
    }
    let input = ready.input();
    if input.relation().context_fingerprint() != context.fingerprint() {
        return Err(GeneratedResidualAffineWhenBadPullbackGateError::WrongContext);
    }
    let expected = context.index_count();
    for actual in [
        input.relation().arity(),
        ready.binding().sector().arity(),
        input
            .target_branch()
            .integer_system_arc()
            .map_or(expected, |system| system.ambient_arity()),
    ] {
        if actual != expected {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::WrongArity { expected, actual },
            );
        }
    }
    if ready.binding().rhs_terms() != ready.private_rhs_proofs().len() {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::PrivateRhsCountMismatch {
                authenticated: ready.binding().rhs_terms(),
                observed: ready.private_rhs_proofs().len(),
            },
        );
    }
    if let Some(retained) = retained {
        if retained.schema != GENERATED_RESIDUAL_AFFINE_WHEN_BAD_PULLBACK_GATE_V1_SCHEMA
            || retained.context_fingerprint.as_ref() != context.fingerprint()
            || retained.source_case_ordinal != ready.binding().source_case_ordinal()
            || retained.source_group_ordinal != ready.binding().source_group_ordinal()
            || retained.pivot_ordinal != ready.binding().pivot_ordinal()
            || retained.target_case_ordinal != ready.binding().target_case_ordinal()
            || !Arc::ptr_eq(&retained.matcher, input.matcher())
            || !Arc::ptr_eq(&retained.relation, input.relation())
            || !Arc::ptr_eq(&retained.target_branch, input.target_branch())
            || input
                .target_branch()
                .integer_system_arc()
                .is_none_or(|system| !Arc::ptr_eq(&retained.integer_system, system))
        {
            return Err(GeneratedResidualAffineWhenBadPullbackGateError::ReadyAuthorityMismatch);
        }
    }
    Ok(())
}

fn validate_descent_proof_binding(
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    proof: GeneratedResidualAffineWhenBadRhsDescentProof,
    rhs_ordinal: usize,
    shift: &crate::IndexShift,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    if proof.is_target_sector() {
        let witness = ready
            .private_target_sector_transcript()
            .witnesses()
            .get(proof.witness_ordinal())
            .copied()
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch {
                    rhs_ordinal,
                },
            )?;
        if witness.rhs_ordinal() != rhs_ordinal {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch {
                    rhs_ordinal,
                },
            );
        }
    } else {
        let witness = ready
            .private_witnesses()
            .get(proof.witness_ordinal())
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch {
                    rhs_ordinal,
                },
            )?;
        if witness.rhs_ordinal() != rhs_ordinal || witness.rhs_shift() != shift {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch {
                    rhs_ordinal,
                },
            );
        }
    }
    Ok(())
}

fn activation_obligation_for_boundary(
    ready: &GeneratedResidualAffineWhenBadDescentReady,
    proof: GeneratedResidualAffineWhenBadRhsDescentProof,
    rhs_ordinal: usize,
    coordinate: usize,
    boundary_value: i64,
    hazard: WhenBadBoundaryHazardKind,
) -> Result<
    Option<GeneratedResidualAffineActivationObligationLocator>,
    GeneratedResidualAffineWhenBadPullbackGateError,
> {
    if !proof.is_target_sector() || hazard != WhenBadBoundaryHazardKind::InactiveSectorActivation {
        return Ok(None);
    }
    let witness_ordinal = proof.witness_ordinal();
    let witness = ready
        .private_target_sector_transcript()
        .witnesses()
        .get(witness_ordinal)
        .copied()
        .ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::DescentProofMismatch { rhs_ordinal },
        )?;
    let obligations = ready
        .private_target_sector_transcript()
        .symbolic_activation_obligations(witness)
        .ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                rhs_ordinal,
            },
        )?;
    let Ok(ordinal) =
        obligations.binary_search_by_key(&coordinate, |obligation| obligation.position())
    else {
        return Ok(None);
    };
    let obligation = obligations[ordinal];
    if boundary_value < obligation.first() || boundary_value > obligation.last() {
        return Ok(None);
    }
    Ok(Some(GeneratedResidualAffineActivationObligationLocator {
        target_witness_ordinal: witness_ordinal,
        obligation_ordinal_within_witness: ordinal,
    }))
}

fn validate_activation_obligation_event_coverage(
    _ready: &GeneratedResidualAffineWhenBadDescentReady,
    events: &[GeneratedResidualAffineWhenBadPullbackGateEvent],
    census: &HazardCensus,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    let observed = events
        .iter()
        .filter(|event| event.provenance.activation_obligation.is_some())
        .count();
    if observed != census.activation_obligation_boundary_values {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::ActivationObligationMismatch {
                rhs_ordinal: 0,
            },
        );
    }
    // A locator is manufactured only by exact coordinate/range matching
    // against the authenticated obligation slice. Duplicate obligation
    // coordinates were rejected during census. The exact total range
    // cardinality therefore proves complete coverage without an
    // obligations-by-events quadratic scan.
    Ok(())
}

fn boundary_source_polynomial(
    context: &ParametricCoefficientContext,
    coordinate: usize,
    boundary_value: i64,
    limits: ResidualUnitAffinePolynomialCompositionLimits,
) -> Result<ParametricPolynomial, GeneratedResidualAffineWhenBadPullbackGateError> {
    let index = context.index(coordinate)?;
    let value = context.integer(boundary_value);
    let difference = context.sub_with_limits(&index, &value, limits.exact_algebra)?;
    Ok(context.numerator_condition_with_limits(&difference, limits.exact_algebra)?)
}

fn boundary_source_shape(
    context: &ParametricCoefficientContext,
    boundary_value: i64,
) -> Result<PolynomialShape, GeneratedResidualAffineWhenBadPullbackGateError> {
    let terms = 1usize + usize::from(boundary_value != 0);
    let variables = checked_add(
        "generated affine pullback/gate boundary source variables",
        context.base().variables().len(),
        context.index_count(),
    )?;
    let exponent_entries = checked_mul(
        "generated affine pullback/gate boundary source exponent entries",
        terms,
        variables,
    )?;
    let integer_bits = checked_add(
        "generated affine pullback/gate boundary source integer bits",
        1,
        if boundary_value == 0 {
            0
        } else {
            usize::try_from(i64::BITS - boundary_value.unsigned_abs().leading_zeros()).map_err(
                |_| GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
                    resource: "generated affine pullback/gate boundary source integer bits",
                },
            )?
        },
    )?;
    Ok(PolynomialShape {
        terms,
        exponent_entries,
        integer_bits,
    })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PolynomialShape {
    terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
}

fn polynomial_shape(
    polynomial: &ParametricPolynomial,
) -> Result<PolynomialShape, GeneratedResidualAffineWhenBadPullbackGateError> {
    let mut integer_bits = 0usize;
    for coefficient in &polynomial.raw().coefficients {
        integer_bits = checked_add(
            "generated affine pullback/gate polynomial integer bits",
            integer_bits,
            integer_magnitude_bits(coefficient)?,
        )?;
    }
    Ok(PolynomialShape {
        terms: polynomial.raw().nterms(),
        exponent_entries: polynomial.raw().exponents.len(),
        integer_bits,
    })
}

fn coefficient_numerator_shape(
    coefficient: &ParametricCoefficient,
) -> Result<PolynomialShape, GeneratedResidualAffineWhenBadPullbackGateError> {
    let raw = &coefficient.raw().numerator;
    let mut integer_bits = 0usize;
    for value in &raw.coefficients {
        integer_bits = checked_add(
            "generated affine pullback/gate numerator integer bits",
            integer_bits,
            integer_magnitude_bits(value)?,
        )?;
    }
    Ok(PolynomialShape {
        terms: raw.nterms(),
        exponent_entries: raw.exponents.len(),
        integer_bits,
    })
}

fn coefficient_numerator_depends_on_indices(
    context: &ParametricCoefficientContext,
    coefficient: &ParametricCoefficient,
) -> bool {
    let first_index = context.base().variables().len();
    coefficient
        .raw()
        .numerator
        .exponents_iter()
        .any(|exponents| {
            exponents[first_index..]
                .iter()
                .any(|exponent| *exponent != 0)
        })
}

fn validate_numerator_free_support(
    context: &ParametricCoefficientContext,
    coefficient: &ParametricCoefficient,
    free_positions: &[usize],
    rhs_ordinal: usize,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    let first_index = context.base().variables().len();
    for exponents in coefficient.raw().numerator.exponents_iter() {
        for (position, exponent) in exponents[first_index..].iter().copied().enumerate() {
            if exponent != 0 && free_positions.binary_search(&position).is_err() {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::NonfreeNumeratorSupport {
                        rhs_ordinal,
                    },
                );
            }
        }
    }
    Ok(())
}

fn precharge_source_shapes(
    stats: &mut GeneratedResidualAffineWhenBadPullbackGateStats,
    source: PolynomialShape,
    gate: PolynomialShape,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    stats.total_source_terms = bounded_add(
        "generated affine pullback/gate total source terms",
        stats.total_source_terms,
        checked_add(
            "generated affine pullback/gate total source terms",
            source.terms,
            gate.terms,
        )?,
        limits.max_total_source_terms,
    )?;
    stats.total_source_exponent_entries = bounded_add(
        "generated affine pullback/gate total source exponent entries",
        stats.total_source_exponent_entries,
        checked_add(
            "generated affine pullback/gate total source exponent entries",
            source.exponent_entries,
            gate.exponent_entries,
        )?,
        limits.max_total_source_exponent_entries,
    )?;
    stats.total_source_integer_bits = bounded_add(
        "generated affine pullback/gate total source integer bits",
        stats.total_source_integer_bits,
        checked_add(
            "generated affine pullback/gate total source integer bits",
            source.integer_bits,
            gate.integer_bits,
        )?,
        limits.max_total_source_integer_bits,
    )?;
    Ok(())
}

fn remaining_composition_limits(
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
    stats: &GeneratedResidualAffineWhenBadPullbackGateStats,
) -> Result<
    ResidualUnitAffinePolynomialCompositionLimits,
    GeneratedResidualAffineWhenBadPullbackGateError,
> {
    let mut effective = limits.polynomial_composition;
    // Source terms were already charged for this event.  The child sees its
    // own nested source cap; aggregate source caps were enforced before this
    // projection and include the numerator-copy work as well.
    effective.max_expanded_contributions = effective.max_expanded_contributions.min(remaining(
        "generated affine pullback/gate total expanded contributions",
        limits.max_total_expanded_contributions,
        stats.total_expanded_contributions,
    )?);
    effective.max_output_terms = effective
        .max_output_terms
        .min(remaining(
            "generated affine pullback/gate total output-term bound",
            limits.max_total_output_term_bound,
            stats.total_output_term_bound,
        )?)
        .min(remaining(
            "generated affine pullback/gate total output terms",
            limits.max_total_output_terms,
            stats.total_output_terms,
        )?);
    effective.max_output_exponent_entries = effective
        .max_output_exponent_entries
        .min(remaining(
            "generated affine pullback/gate total output exponent-entry bound",
            limits.max_total_output_exponent_entry_bound,
            stats.total_output_exponent_entry_bound,
        )?)
        .min(remaining(
            "generated affine pullback/gate total output exponent entries",
            limits.max_total_output_exponent_entries,
            stats.total_output_exponent_entries,
        )?);
    effective.max_power_calls = effective.max_power_calls.min(remaining(
        "generated affine pullback/gate total power calls",
        limits.max_total_power_calls,
        stats.total_power_calls,
    )?);
    effective.max_native_power_heap_pairs = effective.max_native_power_heap_pairs.min(remaining(
        "generated affine pullback/gate total native power heap pairs",
        limits.max_total_native_power_heap_pairs,
        stats.total_native_power_heap_pairs,
    )?);
    effective.max_multiplication_term_pairs =
        effective.max_multiplication_term_pairs.min(remaining(
            "generated affine pullback/gate total multiplication term pairs",
            limits.max_total_multiplication_term_pairs,
            stats.total_multiplication_term_pairs,
        )?);
    effective.max_addition_term_visits = effective.max_addition_term_visits.min(remaining(
        "generated affine pullback/gate total addition term visits",
        limits.max_total_addition_term_visits,
        stats.total_addition_term_visits,
    )?);
    effective.max_integer_bit_work = effective.max_integer_bit_work.min(remaining(
        "generated affine pullback/gate total integer-bit work",
        limits.max_total_integer_bit_work,
        stats.total_integer_bit_work,
    )?);
    Ok(effective)
}

fn same_preflight_work(
    expected: ResidualUnitAffinePolynomialCompositionStats,
    actual: ResidualUnitAffinePolynomialCompositionStats,
) -> bool {
    expected.source_terms() == actual.source_terms()
        && expected.source_exponent_entries() == actual.source_exponent_entries()
        && expected.expanded_contribution_bound() == actual.expanded_contribution_bound()
        && expected.output_exponent_entry_bound() == actual.output_exponent_entry_bound()
        && expected.power_calls() == actual.power_calls()
        && expected.native_power_heap_pair_bound() == actual.native_power_heap_pair_bound()
        && expected.multiplication_term_pair_bound() == actual.multiplication_term_pair_bound()
        && expected.addition_term_visit_bound() == actual.addition_term_visit_bound()
        && expected.largest_kronecker_exponent_bits() == actual.largest_kronecker_exponent_bits()
        && expected.largest_integer_coefficient_bit_bound()
            == actual.largest_integer_coefficient_bit_bound()
        && expected.native_integer_bit_work_bound() == actual.native_integer_bit_work_bound()
        && expected.integer_bit_work_bound() == actual.integer_bit_work_bound()
}

fn aggregate_composition_stats(
    aggregate: &mut GeneratedResidualAffineWhenBadPullbackGateStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    aggregate.total_expanded_contributions = bounded_add(
        "generated affine pullback/gate total expanded contributions",
        aggregate.total_expanded_contributions,
        item.expanded_contribution_bound(),
        limits.max_total_expanded_contributions,
    )?;
    aggregate.total_output_term_bound = bounded_add(
        "generated affine pullback/gate total output-term bound",
        aggregate.total_output_term_bound,
        item.expanded_contribution_bound(),
        limits.max_total_output_term_bound,
    )?;
    aggregate.total_output_terms = bounded_add(
        "generated affine pullback/gate total output terms",
        aggregate.total_output_terms,
        item.output_terms(),
        limits.max_total_output_terms,
    )?;
    aggregate.total_output_exponent_entry_bound = bounded_add(
        "generated affine pullback/gate total output exponent-entry bound",
        aggregate.total_output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        limits.max_total_output_exponent_entry_bound,
    )?;
    aggregate.total_output_exponent_entries = bounded_add(
        "generated affine pullback/gate total output exponent entries",
        aggregate.total_output_exponent_entries,
        item.output_exponent_entries(),
        limits.max_total_output_exponent_entries,
    )?;
    aggregate.total_power_calls = bounded_add(
        "generated affine pullback/gate total power calls",
        aggregate.total_power_calls,
        item.power_calls(),
        limits.max_total_power_calls,
    )?;
    aggregate.total_native_power_heap_pairs = bounded_add(
        "generated affine pullback/gate total native power heap pairs",
        aggregate.total_native_power_heap_pairs,
        item.native_power_heap_pair_bound(),
        limits.max_total_native_power_heap_pairs,
    )?;
    aggregate.total_multiplication_term_pairs = bounded_add(
        "generated affine pullback/gate total multiplication term pairs",
        aggregate.total_multiplication_term_pairs,
        item.multiplication_term_pair_bound(),
        limits.max_total_multiplication_term_pairs,
    )?;
    aggregate.total_addition_term_visits = bounded_add(
        "generated affine pullback/gate total addition term visits",
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
        "generated affine pullback/gate total native integer-bit work",
        aggregate.total_native_integer_bit_work,
        item.native_integer_bit_work_bound(),
        limits.max_total_native_integer_bit_work,
    )?;
    aggregate.total_integer_bit_work = bounded_add(
        "generated affine pullback/gate total integer-bit work",
        aggregate.total_integer_bit_work,
        item.integer_bit_work_bound(),
        limits.max_total_integer_bit_work,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RetainedEventByteEnvelope {
    pullback: usize,
    numerator_gate: usize,
}

fn preflight_retained_pullback_envelope(
    retained_envelope: &mut usize,
    stats: &GeneratedResidualAffineWhenBadPullbackGateStats,
    preflight: ResidualUnitAffinePolynomialCompositionStats,
    gate: PolynomialShape,
    variables: usize,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<RetainedEventByteEnvelope, GeneratedResidualAffineWhenBadPullbackGateError> {
    let pullback = PolynomialShape {
        terms: preflight.expanded_contribution_bound(),
        exponent_entries: preflight.output_exponent_entry_bound(),
        integer_bits: checked_mul(
            "generated affine pullback/gate prospective retained polynomial integer bits",
            preflight.expanded_contribution_bound(),
            preflight.largest_integer_coefficient_bit_bound(),
        )?,
    };
    let prospective_terms = checked_add(
        "generated affine pullback/gate prospective retained polynomial terms",
        pullback.terms,
        gate.terms,
    )?;
    check_limit(
        "generated affine pullback/gate retained polynomial terms",
        checked_add(
            "generated affine pullback/gate retained polynomial terms",
            stats.retained_polynomial_terms,
            prospective_terms,
        )?,
        limits.max_retained_polynomial_terms,
    )?;
    let prospective_exponents = checked_add(
        "generated affine pullback/gate prospective retained polynomial exponent entries",
        pullback.exponent_entries,
        gate.exponent_entries,
    )?;
    check_limit(
        "generated affine pullback/gate retained polynomial exponent entries",
        checked_add(
            "generated affine pullback/gate retained polynomial exponent entries",
            stats.retained_polynomial_exponent_entries,
            prospective_exponents,
        )?,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    let prospective_bits = checked_add(
        "generated affine pullback/gate prospective retained polynomial integer bits",
        pullback.integer_bits,
        gate.integer_bits,
    )?;
    check_limit(
        "generated affine pullback/gate retained polynomial integer bits",
        checked_add(
            "generated affine pullback/gate retained polynomial integer bits",
            stats.retained_polynomial_integer_bits,
            prospective_bits,
        )?,
        limits.max_retained_polynomial_integer_bits,
    )?;
    let pullback_byte_envelope = polynomial_owned_byte_envelope(pullback, variables)?;
    let gate_byte_envelope = polynomial_owned_byte_envelope(gate, variables)?;
    let event_polynomial_envelope = checked_add(
        "generated affine pullback/gate retained bytes",
        pullback_byte_envelope,
        gate_byte_envelope,
    )?;
    let requested = checked_add(
        "generated affine pullback/gate retained bytes",
        *retained_envelope,
        event_polynomial_envelope,
    )?;
    check_limit(
        "generated affine pullback/gate retained bytes",
        requested,
        limits.max_retained_bytes,
    )?;
    *retained_envelope = requested;
    Ok(RetainedEventByteEnvelope {
        pullback: pullback_byte_envelope,
        numerator_gate: gate_byte_envelope,
    })
}

fn retain_polynomial_shape(
    polynomial: &ParametricPolynomial,
    stats: &mut GeneratedResidualAffineWhenBadPullbackGateStats,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<PolynomialShape, GeneratedResidualAffineWhenBadPullbackGateError> {
    let shape = polynomial_shape(polynomial)?;
    retain_polynomial_shape_precomputed(shape, stats, limits)?;
    Ok(shape)
}

fn retain_polynomial_shape_precomputed(
    shape: PolynomialShape,
    stats: &mut GeneratedResidualAffineWhenBadPullbackGateStats,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    stats.retained_polynomial_terms = bounded_add(
        "generated affine pullback/gate retained polynomial terms",
        stats.retained_polynomial_terms,
        shape.terms,
        limits.max_retained_polynomial_terms,
    )?;
    stats.retained_polynomial_exponent_entries = bounded_add(
        "generated affine pullback/gate retained polynomial exponent entries",
        stats.retained_polynomial_exponent_entries,
        shape.exponent_entries,
        limits.max_retained_polynomial_exponent_entries,
    )?;
    stats.retained_polynomial_integer_bits = bounded_add(
        "generated affine pullback/gate retained polynomial integer bits",
        stats.retained_polynomial_integer_bits,
        shape.integer_bits,
        limits.max_retained_polynomial_integer_bits,
    )?;
    Ok(())
}

fn validate_polynomial_actual_byte_envelope(
    polynomial: &ParametricPolynomial,
    admitted: usize,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    let observed = polynomial.owned_retained_byte_bound().ok_or(
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
            resource: "generated affine pullback/gate retained polynomial bytes",
        },
    )?;
    if observed > admitted {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::RetainedByteEnvelopeExceeded {
                observed,
                admitted,
            },
        );
    }
    Ok(())
}

fn polynomial_owned_byte_envelope(
    shape: PolynomialShape,
    _variables: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    let coefficient_capacity = capacity_envelope(shape.terms)?;
    let exponent_capacity = capacity_envelope(shape.exponent_entries)?;
    checked_add(
        "generated affine pullback/gate retained polynomial bytes",
        size_of::<ParametricPolynomial>(),
        checked_add(
            "generated affine pullback/gate retained polynomial bytes",
            checked_mul(
                "generated affine pullback/gate retained polynomial bytes",
                coefficient_capacity,
                size_of::<Integer>(),
            )?,
            checked_add(
                "generated affine pullback/gate retained polynomial bytes",
                checked_mul(
                    "generated affine pullback/gate retained polynomial bytes",
                    exponent_capacity,
                    size_of::<u16>(),
                )?,
                checked_add(
                    "generated affine pullback/gate retained polynomial bytes",
                    shape.integer_bits.checked_add(7).ok_or(
                        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
                            resource: "generated affine pullback/gate retained polynomial bytes",
                        },
                    )? / 8,
                    checked_mul(
                        "generated affine pullback/gate retained polynomial bytes",
                        shape.terms,
                        size_of::<usize>(),
                    )?,
                )?,
            )?,
        )?,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompositionPlanRetainedPreflight {
    shape: CompositionPlanRetainedShape,
    large_coefficients: usize,
    large_significant_bytes: usize,
}

#[cfg(test)]
thread_local! {
    static COMPOSITION_PLAN_PREFLIGHT_INTEGER_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_composition_plan_preflight_integer_scan() {
    #[cfg(test)]
    COMPOSITION_PLAN_PREFLIGHT_INTEGER_SCANS.with(|scans| scans.set(scans.get() + 1));
}

fn preflight_composition_plan_retained_shape(
    context: &ParametricCoefficientContext,
    integer_system: &ResidualAffineIntegerSystemCertificate,
    plan_limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<CompositionPlanRetainedPreflight, GeneratedResidualAffineWhenBadPullbackGateError> {
    let map = integer_system.affine_map().ok_or(
        GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
            resource: "authenticated affine map",
        },
    )?;
    let ambient_arity = context.index_count();
    if map.ambient_arity() != ambient_arity {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::WrongArity {
                expected: ambient_arity,
                actual: map.ambient_arity(),
            },
        );
    }
    let free_positions = map.free_positions().len();
    let nonfree_positions = map.pivot_positions().len();
    if free_positions.checked_add(nonfree_positions) != Some(ambient_arity) {
        return Err(
            GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                resource: "authenticated free/nonfree partition",
            },
        );
    }
    let variables = checked_add(
        "generated affine pullback/gate retained composition-plan variables",
        context.base().variables().len(),
        ambient_arity,
    )?;
    check_composition_plan_limit(
        "composition variables",
        variables,
        plan_limits.max_variables,
    )?;
    check_composition_plan_limit("full-point images", variables, plan_limits.max_full_images)?;
    let geometry_entries_inspected = checked_add(
        "generated affine pullback/gate composition-plan geometry entries inspected",
        ambient_arity,
        checked_mul(
            "generated affine pullback/gate composition-plan geometry entries inspected",
            ambient_arity,
            ambient_arity,
        )?,
    )?;
    check_composition_plan_limit(
        "affine geometry entries inspected",
        geometry_entries_inspected,
        plan_limits.max_geometry_entries_inspected,
    )?;
    let linear_support_entries = checked_mul(
        "generated affine pullback/gate retained composition-plan linear support",
        ambient_arity,
        free_positions,
    )?;
    let geometry_entries_retained = checked_add(
        "generated affine pullback/gate retained composition-plan geometry entries",
        ambient_arity,
        linear_support_entries,
    )?;
    check_composition_plan_limit(
        "affine geometry entries retained",
        geometry_entries_retained,
        plan_limits.max_geometry_entries_retained,
    )?;
    check_composition_plan_limit(
        "affine support entries retained",
        geometry_entries_retained,
        plan_limits.max_support_entries_retained,
    )?;
    let mut total_image_terms = context.base().variables().len();
    let mut total_image_integer_bits = context.base().variables().len();
    let mut large_coefficients = 0usize;
    let mut large_significant_bytes = 0usize;
    check_composition_plan_limit(
        "total image terms",
        total_image_terms,
        plan_limits.max_total_image_terms,
    )?;
    if total_image_terms != 0 {
        check_composition_plan_limit(
            "image integer coefficient bits",
            1,
            plan_limits.max_image_integer_bits,
        )?;
    }
    check_composition_plan_limit(
        "total image integer bits",
        total_image_integer_bits,
        plan_limits.max_total_image_integer_bits,
    )?;
    for row in 0..ambient_arity {
        let constant = map.constant(row).ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                resource: "authenticated affine constants",
            },
        )?;
        observe_composition_plan_preflight_integer_scan();
        let constant_bits = integer_magnitude_bits(constant)?;
        check_composition_plan_limit(
            "image integer coefficient bits",
            constant_bits,
            plan_limits.max_image_integer_bits,
        )?;
        total_image_integer_bits = checked_add(
            "generated affine pullback/gate retained composition-plan integer bits",
            total_image_integer_bits,
            constant_bits,
        )?;
        check_composition_plan_limit(
            "total image integer bits",
            total_image_integer_bits,
            plan_limits.max_total_image_integer_bits,
        )?;
        total_image_terms = checked_add(
            "generated affine pullback/gate retained composition-plan image terms",
            total_image_terms,
            usize::from(!constant.is_zero()),
        )?;
        check_composition_plan_limit(
            "total image terms",
            total_image_terms,
            plan_limits.max_total_image_terms,
        )?;
        charge_large_image_integer(
            constant,
            constant_bits,
            &mut large_coefficients,
            &mut large_significant_bytes,
        )?;
        for &free_position in map.free_positions() {
            if free_position >= ambient_arity {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                        resource: "authenticated free position",
                    },
                );
            }
            let coefficient = map.linear_coefficient(row, free_position).ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                    resource: "authenticated affine linear coefficient",
                },
            )?;
            observe_composition_plan_preflight_integer_scan();
            let coefficient_bits = integer_magnitude_bits(coefficient)?;
            check_composition_plan_limit(
                "image integer coefficient bits",
                coefficient_bits,
                plan_limits.max_image_integer_bits,
            )?;
            total_image_integer_bits = checked_add(
                "generated affine pullback/gate retained composition-plan integer bits",
                total_image_integer_bits,
                coefficient_bits,
            )?;
            check_composition_plan_limit(
                "total image integer bits",
                total_image_integer_bits,
                plan_limits.max_total_image_integer_bits,
            )?;
            total_image_terms = checked_add(
                "generated affine pullback/gate retained composition-plan image terms",
                total_image_terms,
                usize::from(!coefficient.is_zero()),
            )?;
            check_composition_plan_limit(
                "total image terms",
                total_image_terms,
                plan_limits.max_total_image_terms,
            )?;
            charge_large_image_integer(
                coefficient,
                coefficient_bits,
                &mut large_coefficients,
                &mut large_significant_bytes,
            )?;
        }
    }
    let total_image_exponent_entries = checked_mul(
        "generated affine pullback/gate retained composition-plan image exponents",
        total_image_terms,
        variables,
    )?;
    check_composition_plan_limit(
        "total image exponent entries",
        total_image_exponent_entries,
        plan_limits.max_total_image_exponent_entries,
    )?;
    Ok(CompositionPlanRetainedPreflight {
        shape: CompositionPlanRetainedShape {
            free_positions,
            nonfree_positions,
            linear_support_entries,
            full_images: variables,
            image_term_counts: variables,
            image_coefficient_growth_bits: variables,
            total_image_terms,
            total_image_exponent_entries,
            total_image_integer_bits,
        },
        large_coefficients,
        large_significant_bytes,
    })
}

fn check_composition_plan_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineWhenBadPullbackGateError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource,
                    requested,
                    limit,
                },
            ),
        )
    } else {
        Ok(())
    }
}

fn charge_large_image_integer(
    value: &Integer,
    magnitude_bits: usize,
    large_coefficients: &mut usize,
    large_significant_bytes: &mut usize,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    if !matches!(value, Integer::Large(_)) || value.is_zero() {
        return Ok(());
    }
    *large_coefficients = checked_add(
        "generated affine pullback/gate retained composition-plan large coefficients",
        *large_coefficients,
        1,
    )?;
    let significant_bytes = magnitude_bits.checked_add(7).ok_or(
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
            resource: "generated affine pullback/gate retained composition-plan large integer bytes",
        },
    )? / 8;
    *large_significant_bytes = checked_add(
        "generated affine pullback/gate retained composition-plan large integer bytes",
        *large_significant_bytes,
        significant_bytes,
    )?;
    Ok(())
}

fn admit_retained_envelope_before_plan(
    pre_plan_retained_envelope: usize,
    plan_retained_envelope: usize,
    context_fingerprint_bytes: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    let retained_envelope = checked_add(
        "generated affine pullback/gate retained bytes",
        pre_plan_retained_envelope,
        plan_retained_envelope,
    )?;
    let retained_envelope = checked_add(
        "generated affine pullback/gate retained bytes",
        retained_envelope,
        conservative_arc_str_byte_envelope(context_fingerprint_bytes)?,
    )?;
    check_limit(
        "generated affine pullback/gate retained bytes",
        retained_envelope,
        limit,
    )?;
    Ok(retained_envelope)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CompositionPlanRetainedShape {
    free_positions: usize,
    nonfree_positions: usize,
    linear_support_entries: usize,
    full_images: usize,
    image_term_counts: usize,
    image_coefficient_growth_bits: usize,
    total_image_terms: usize,
    total_image_exponent_entries: usize,
    total_image_integer_bits: usize,
}

impl CompositionPlanRetainedShape {
    fn from_stats(
        stats: ResidualAffineCompositionPlanStats,
        ambient_arity: usize,
    ) -> Result<Self, GeneratedResidualAffineWhenBadPullbackGateError> {
        if stats.full_images() != stats.variables()
            || stats.geometry_entries_retained() != stats.support_entries_retained()
        {
            return Err(
                GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                    resource: "full-image or retained-geometry cardinality",
                },
            );
        }
        let linear_support_entries = stats
            .support_entries_retained()
            .checked_sub(ambient_arity)
            .ok_or(
                GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                    resource: "linear-support cardinality",
                },
            )?;
        let free_positions = if ambient_arity == 0 {
            if linear_support_entries != 0 {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                        resource: "zero-arity linear support",
                    },
                );
            }
            0
        } else {
            if linear_support_entries % ambient_arity != 0 {
                return Err(
                    GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                        resource: "rectangular linear support",
                    },
                );
            }
            linear_support_entries / ambient_arity
        };
        let nonfree_positions = ambient_arity.checked_sub(free_positions).ok_or(
            GeneratedResidualAffineWhenBadPullbackGateError::CompositionPlanStatsMismatch {
                resource: "free/nonfree partition",
            },
        )?;
        Ok(Self {
            free_positions,
            nonfree_positions,
            linear_support_entries,
            full_images: stats.full_images(),
            image_term_counts: stats.variables(),
            image_coefficient_growth_bits: stats.variables(),
            total_image_terms: stats.total_image_terms(),
            total_image_exponent_entries: stats.total_image_exponent_entries(),
            total_image_integer_bits: stats.total_image_integer_bits(),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct CompositionPlanRetainedByteCensus {
    outer_plan_arc: usize,
    core_plan_arc: usize,
    free_positions: usize,
    nonfree_positions: usize,
    linear_support: usize,
    full_images: usize,
    image_term_counts: usize,
    image_coefficient_growth_bits: usize,
    image_coefficients: usize,
    image_exponents: usize,
    image_integer_payload: usize,
}

impl CompositionPlanRetainedByteCensus {
    fn total(self) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
        let mut total = 0usize;
        for bytes in [
            self.outer_plan_arc,
            self.core_plan_arc,
            self.free_positions,
            self.nonfree_positions,
            self.linear_support,
            self.full_images,
            self.image_term_counts,
            self.image_coefficient_growth_bits,
            self.image_coefficients,
            self.image_exponents,
            self.image_integer_payload,
        ] {
            total = checked_add(
                "generated affine pullback/gate retained composition-plan bytes",
                total,
                bytes,
            )?;
        }
        Ok(total)
    }
}

fn composition_plan_retained_byte_census(
    preflight: CompositionPlanRetainedPreflight,
) -> Result<CompositionPlanRetainedByteCensus, GeneratedResidualAffineWhenBadPullbackGateError> {
    let shape = preflight.shape;
    let large_integer_limb_slack = checked_mul(
        "generated affine pullback/gate retained composition-plan bytes",
        preflight.large_coefficients,
        size_of::<usize>(),
    )?;
    Ok(CompositionPlanRetainedByteCensus {
        outer_plan_arc: conservative_arc_allocation_byte_envelope(
            size_of::<ResidualAffineCompositionPlan>(),
            align_of::<ResidualAffineCompositionPlan>(),
        )?,
        core_plan_arc: residual_affine_composition_core_arc_byte_envelope()?,
        free_positions: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.free_positions)?,
            size_of::<usize>(),
        )?,
        nonfree_positions: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.nonfree_positions)?,
            size_of::<usize>(),
        )?,
        linear_support: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.linear_support_entries)?,
            size_of::<bool>(),
        )?,
        full_images: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.full_images)?,
            size_of::<CoefficientPolynomial>(),
        )?,
        image_term_counts: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.image_term_counts)?,
            size_of::<usize>(),
        )?,
        image_coefficient_growth_bits: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.image_coefficient_growth_bits)?,
            size_of::<usize>(),
        )?,
        image_coefficients: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.total_image_terms)?,
            size_of::<Integer>(),
        )?,
        image_exponents: checked_mul(
            "generated affine pullback/gate retained composition-plan bytes",
            capacity_envelope(shape.total_image_exponent_entries)?,
            size_of::<u16>(),
        )?,
        image_integer_payload: checked_add(
            "generated affine pullback/gate retained composition-plan bytes",
            preflight.large_significant_bytes,
            large_integer_limb_slack,
        )?,
    })
}

fn residual_affine_composition_core_arc_byte_envelope()
-> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    // The core type is deliberately private to `parametric_coefficient`.
    // Sum every known field and admit worst-case padding independently, so
    // this remains an upper envelope even though Rust may reorder fields.
    let fields = [
        (size_of::<&'static str>(), align_of::<&'static str>()),
        (size_of::<Arc<str>>(), align_of::<Arc<str>>()),
        (size_of::<usize>(), align_of::<usize>()),
        (size_of::<Vec<usize>>(), align_of::<Vec<usize>>()),
        (size_of::<Vec<usize>>(), align_of::<Vec<usize>>()),
        (size_of::<Vec<bool>>(), align_of::<Vec<bool>>()),
        (
            size_of::<Vec<CoefficientPolynomial>>(),
            align_of::<Vec<CoefficientPolynomial>>(),
        ),
        (size_of::<Vec<usize>>(), align_of::<Vec<usize>>()),
        (size_of::<Vec<usize>>(), align_of::<Vec<usize>>()),
        (
            size_of::<ResidualUnitAffineCompositionPlanLimits>(),
            align_of::<ResidualUnitAffineCompositionPlanLimits>(),
        ),
        (
            size_of::<ResidualAffineCompositionPlanStats>(),
            align_of::<ResidualAffineCompositionPlanStats>(),
        ),
    ];
    let mut payload = 0usize;
    let mut payload_alignment = 1usize;
    for (field_bytes, field_alignment) in fields {
        payload = checked_add(
            "generated affine pullback/gate retained composition-plan core bytes",
            payload,
            field_bytes,
        )?;
        payload = checked_add(
            "generated affine pullback/gate retained composition-plan core bytes",
            payload,
            field_alignment.saturating_sub(1),
        )?;
        payload_alignment = payload_alignment.max(field_alignment);
    }
    payload = checked_add(
        "generated affine pullback/gate retained composition-plan core bytes",
        payload,
        payload_alignment.saturating_sub(1),
    )?;
    conservative_arc_allocation_byte_envelope(payload, payload_alignment)
}

#[derive(Clone, Copy, Debug, Default)]
struct PayloadComparisonCensus {
    units: usize,
    bytes: usize,
    integer_bits: usize,
}

struct PayloadComparisonBudget {
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
    census: PayloadComparisonCensus,
}

impl PayloadComparisonBudget {
    const fn new(limits: GeneratedResidualAffineWhenBadPullbackGateLimits) -> Self {
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
    ) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
        self.census.units = bounded_add(
            "generated affine pullback/gate payload comparison units",
            self.census.units,
            additional.units,
            self.limits.max_payload_comparison_units,
        )?;
        self.census.bytes = bounded_add(
            "generated affine pullback/gate payload comparison bytes",
            self.census.bytes,
            additional.bytes,
            self.limits.max_payload_comparison_bytes,
        )?;
        self.census.integer_bits = bounded_add(
            "generated affine pullback/gate payload comparison integer bits",
            self.census.integer_bits,
            additional.integer_bits,
            self.limits.max_payload_comparison_integer_bits,
        )?;
        Ok(())
    }

    fn add_units(
        &mut self,
        units: usize,
    ) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
        self.charge(PayloadComparisonCensus {
            units,
            ..PayloadComparisonCensus::default()
        })
    }

    fn add_string(
        &mut self,
        value: &str,
    ) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
        self.charge(PayloadComparisonCensus {
            units: value.len(),
            bytes: value.len(),
            integer_bits: 0,
        })
    }

    fn add_polynomial(
        &mut self,
        polynomial: &ParametricPolynomial,
    ) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
        let raw = polynomial.raw();
        self.charge(PayloadComparisonCensus {
            units: checked_add(
                "generated affine pullback/gate payload comparison units",
                checked_add(
                    "generated affine pullback/gate payload comparison units",
                    raw.coefficients.len(),
                    raw.exponents.len(),
                )?,
                raw.variables.len(),
            )?,
            bytes: checked_add(
                "generated affine pullback/gate payload comparison bytes",
                size_of_val(raw.coefficients.as_slice()),
                checked_add(
                    "generated affine pullback/gate payload comparison bytes",
                    size_of_val(raw.exponents.as_slice()),
                    checked_mul(
                        "generated affine pullback/gate payload comparison bytes",
                        raw.variables.len(),
                        size_of::<PolyVariable>(),
                    )?,
                )?,
            )?,
            integer_bits: 0,
        })?;
        for coefficient in &raw.coefficients {
            self.charge(PayloadComparisonCensus {
                units: 1,
                bytes: size_of::<Integer>(),
                integer_bits: integer_magnitude_bits(coefficient)?,
            })?;
        }
        Ok(())
    }
}

fn authenticate_payload_comparison_stats(
    certificate: &mut GeneratedResidualAffineWhenBadPullbackGateCertificate,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    let mut budget = PayloadComparisonBudget::new(certificate.limits);
    payload_operand_census(certificate, &mut budget)?;
    payload_operand_census(certificate, &mut budget)?;
    certificate.stats.payload_comparison_units = budget.census.units;
    certificate.stats.payload_comparison_bytes = budget.census.bytes;
    certificate.stats.payload_comparison_integer_bits = budget.census.integer_bits;
    Ok(())
}

fn preflight_payload_comparison(
    left: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
    right: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    debug_assert_eq!(left.limits, right.limits);
    let mut budget = PayloadComparisonBudget::new(left.limits);
    payload_operand_census(left, &mut budget)?;
    payload_operand_census(right, &mut budget)
}

fn payload_operand_census(
    certificate: &GeneratedResidualAffineWhenBadPullbackGateCertificate,
    budget: &mut PayloadComparisonBudget,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    budget.add_string(certificate.context_fingerprint.as_ref())?;
    budget.add_units(scalar_representation_units::<
        GeneratedResidualAffineWhenBadPullbackGateCertificate,
    >())?;
    budget.add_units(scalar_representation_units::<
        GeneratedResidualAffineWhenBadPullbackGateLimits,
    >())?;
    budget.add_units(scalar_representation_units::<
        GeneratedResidualAffineWhenBadPullbackGateStats,
    >())?;
    budget.add_units(certificate.events.len())?;
    for event in &certificate.events {
        budget.add_units(scalar_representation_units::<
            GeneratedResidualAffineWhenBadPullbackGateEvent,
        >())?;
        budget.add_polynomial(&event.pullback)?;
        budget.add_polynomial(event.numerator_gate.polynomial())?;
    }
    Ok(())
}

fn charge_retained_polynomial_display(
    polynomial: &ParametricPolynomial,
    stats: &mut GeneratedResidualAffineWhenBadPullbackGateStats,
    limits: GeneratedResidualAffineWhenBadPullbackGateLimits,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    let remaining_display = remaining(
        "generated affine pullback/gate retained polynomial display bytes",
        limits.max_retained_polynomial_display_bytes,
        stats.retained_polynomial_display_bytes,
    )?;
    let local =
        bounded_polynomial_display_bytes(polynomial, remaining_display).map_err(|requested| {
            GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                resource: "generated affine pullback/gate retained polynomial display bytes",
                requested: stats
                    .retained_polynomial_display_bytes
                    .checked_add(requested)
                    .unwrap_or(usize::MAX),
                limit: limits.max_retained_polynomial_display_bytes,
            }
        })?;
    stats.retained_polynomial_display_bytes = bounded_add(
        "generated affine pullback/gate retained polynomial display bytes",
        stats.retained_polynomial_display_bytes,
        local,
        limits.max_retained_polynomial_display_bytes,
    )?;
    Ok(())
}

fn bounded_polynomial_display_bytes(
    polynomial: &ParametricPolynomial,
    limit: usize,
) -> Result<usize, usize> {
    struct Counter {
        bytes: usize,
        limit: usize,
        overflowed: bool,
    }
    impl fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            let requested = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
            if requested > self.limit {
                self.overflowed = true;
                return Err(fmt::Error);
            }
            self.bytes = requested;
            Ok(())
        }
    }
    let mut counter = Counter {
        bytes: 0,
        limit,
        overflowed: false,
    };
    if write!(&mut counter, "{}", polynomial.raw()).is_err() {
        return Err(if counter.overflowed {
            limit.saturating_add(1)
        } else {
            usize::MAX
        });
    }
    Ok(counter.bytes)
}

fn integer_magnitude_bits(
    value: &Integer,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    let bits = match value {
        Integer::Single(value) => u128::from(i64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u128::from(i128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u128::from(value.significant_bits()),
    };
    usize::try_from(bits).map_err(|_| {
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow {
            resource: "generated affine pullback/gate integer magnitude bits",
        }
    })
}

fn conservative_arc_str_byte_envelope(
    payload_bytes: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    conservative_arc_allocation_byte_envelope(payload_bytes, align_of::<u8>())
}

/// Allocator-independent upper envelope for one distinct `Arc<T>` control
/// block.  Rust keeps the exact layout private, so admit two atomic words,
/// the whole payload, and worst-case padding on both sides.
fn conservative_arc_allocation_byte_envelope(
    payload_bytes: usize,
    payload_alignment: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    let alignment = align_of::<usize>().max(payload_alignment);
    checked_add(
        "generated affine pullback/gate retained bytes",
        checked_mul(
            "generated affine pullback/gate retained bytes",
            2,
            size_of::<usize>(),
        )?,
        checked_add(
            "generated affine pullback/gate retained bytes",
            payload_bytes,
            checked_mul(
                "generated affine pullback/gate retained bytes",
                2,
                alignment.saturating_sub(1),
            )?,
        )?,
    )
}

fn capacity_envelope(
    entries: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    checked_mul(
        "generated affine pullback/gate capacity envelope",
        entries,
        2,
    )
}

fn scalar_representation_units<T>() -> usize {
    let bytes = size_of::<T>();
    let word = size_of::<usize>();
    bytes / word + usize::from(bytes % word != 0)
}

fn remaining(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    limit.checked_sub(consumed).ok_or(
        GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        },
    )
}

fn bounded_add(
    resource: &'static str,
    consumed: usize,
    additional: usize,
    limit: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    let requested = checked_add(resource, consumed, additional)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    left.checked_add(right)
        .ok_or(GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, GeneratedResidualAffineWhenBadPullbackGateError> {
    left.checked_mul(right)
        .ok_or(GeneratedResidualAffineWhenBadPullbackGateError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), GeneratedResidualAffineWhenBadPullbackGateError> {
    if requested > limit {
        Err(
            GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                resource,
                requested,
                limit,
            },
        )
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_limit_projection_is_field_exact() {
        let outer = GeneratedResidualAffineWhenBadLimits::default();
        let child = GeneratedResidualAffineWhenBadPullbackGateLimits::from_outer(outer);
        assert_eq!(child.composition_plan, outer.composition_plan);
        assert_eq!(child.polynomial_composition, outer.polynomial_composition);
        assert_eq!(child.max_boundary_values, outer.max_boundary_values);
        assert_eq!(
            child.max_total_integer_bit_work,
            outer.max_total_integer_bit_work
        );
        assert_eq!(child.max_retained_bytes, outer.max_retained_bytes);
        assert_eq!(
            child.max_payload_comparison_integer_bits,
            outer.max_payload_comparison_integer_bits
        );
    }

    #[test]
    fn redacted_view_contains_no_boundary_coordinate_or_value() {
        let view = AffineBoundaryPullbackView {
            ordinal: 3,
            rhs_ordinal: 2,
            hazard_class: WhenBadBoundaryHazardKind::InactiveSectorActivation,
            pullback_class: AffineBoundaryPullbackClass::FreeIndexDependent,
            numerator_gate_class: AffineWhenBadNumeratorGateClass::FreeIndexNonzero,
        };
        let debug = format!("{view:?}");
        assert!(debug.contains("rhs_ordinal: 2"));
        assert!(!debug.contains("coordinate"));
        assert!(!debug.contains("boundary_value"));
    }

    #[test]
    fn exact_and_one_below_capacity_preflight() {
        assert_eq!(capacity_envelope(17).unwrap(), 34);
        assert!(matches!(
            check_limit("test pullbacks", 17, 16),
            Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                    resource: "test pullbacks",
                    requested: 17,
                    limit: 16,
                }
            )
        ));
    }

    #[test]
    fn nonempty_core_plan_retained_bytes_are_exactly_admitted_and_one_below_rejected() {
        let census = composition_plan_retained_byte_census(CompositionPlanRetainedPreflight {
            shape: CompositionPlanRetainedShape {
                free_positions: 2,
                nonfree_positions: 2,
                linear_support_entries: 8,
                full_images: 7,
                image_term_counts: 7,
                image_coefficient_growth_bits: 7,
                total_image_terms: 15,
                total_image_exponent_entries: 105,
                total_image_integer_bits: 37,
            },
            large_coefficients: 2,
            large_significant_bytes: 32,
        })
        .unwrap();
        assert!(census.outer_plan_arc > 0);
        assert!(census.core_plan_arc > 0);
        assert!(census.free_positions > 0);
        assert!(census.nonfree_positions > 0);
        assert!(census.linear_support > 0);
        assert!(census.full_images > 0);
        assert!(census.image_term_counts > 0);
        assert!(census.image_coefficient_growth_bits > 0);
        assert!(census.image_coefficients > 0);
        assert!(census.image_exponents > 0);
        assert!(census.image_integer_payload > 0);
        let exact = census.total().unwrap();
        check_limit(
            "generated affine pullback/gate retained composition-plan bytes",
            exact,
            exact,
        )
        .unwrap();
        assert!(matches!(
            check_limit(
                "generated affine pullback/gate retained composition-plan bytes",
                exact,
                exact - 1,
            ),
            Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                    resource:
                        "generated affine pullback/gate retained composition-plan bytes",
                    requested,
                    limit,
                }
            ) if requested == exact && limit + 1 == exact
        ));
    }

    #[test]
    fn huge_multi_large_integer_limb_payload_is_exactly_admitted_and_one_below_rejected() {
        use symbolica::domains::integer::MultiPrecisionInteger;

        const LARGE_COEFFICIENTS: usize = 4_096;
        let mut raw_large = MultiPrecisionInteger::from(1);
        raw_large <<= 1_024_u32;
        let large = Integer::Large(raw_large);
        let large_bits = integer_magnitude_bits(&large).unwrap();
        assert_eq!(large_bits, 1_025);
        let mut large_coefficients = 0usize;
        let mut significant_bytes = 0usize;
        charge_large_image_integer(
            &Integer::Single(i64::MAX),
            i64::BITS as usize - 1,
            &mut large_coefficients,
            &mut significant_bytes,
        )
        .unwrap();
        charge_large_image_integer(
            &Integer::Double(i128::MAX),
            i128::BITS as usize - 1,
            &mut large_coefficients,
            &mut significant_bytes,
        )
        .unwrap();
        for _ in 0..LARGE_COEFFICIENTS {
            charge_large_image_integer(
                &large,
                large_bits,
                &mut large_coefficients,
                &mut significant_bytes,
            )
            .unwrap();
        }
        assert_eq!(large_coefficients, LARGE_COEFFICIENTS);
        assert_eq!(significant_bytes, LARGE_COEFFICIENTS * 129);
        let census = composition_plan_retained_byte_census(CompositionPlanRetainedPreflight {
            shape: CompositionPlanRetainedShape {
                free_positions: 32,
                nonfree_positions: 32,
                linear_support_entries: 2_048,
                full_images: 96,
                image_term_counts: 96,
                image_coefficient_growth_bits: 96,
                total_image_terms: LARGE_COEFFICIENTS,
                total_image_exponent_entries: LARGE_COEFFICIENTS * 96,
                total_image_integer_bits: LARGE_COEFFICIENTS * 1_025,
            },
            large_coefficients,
            large_significant_bytes: significant_bytes,
        })
        .unwrap();
        assert_eq!(
            census.image_integer_payload,
            significant_bytes + LARGE_COEFFICIENTS * size_of::<usize>()
        );
        let exact = census.total().unwrap();
        check_limit(
            "generated affine pullback/gate retained composition-plan bytes",
            exact,
            exact,
        )
        .unwrap();
        assert!(matches!(
            check_limit(
                "generated affine pullback/gate retained composition-plan bytes",
                exact,
                exact - 1,
            ),
            Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                    resource:
                        "generated affine pullback/gate retained composition-plan bytes",
                    requested,
                    limit,
                }
            ) if requested == exact && limit + 1 == exact
        ));
    }

    #[test]
    fn generated_ready_path_replays_and_rejects_plan_one_below_before_child_construction() {
        let (context, ready) = crate::generated_residual_affine_when_bad_compilation_tests::generated_ready_fixture_for_pullback_gate();
        let limits = GeneratedResidualAffineWhenBadPullbackGateLimits::from_outer(
            GeneratedResidualAffineWhenBadLimits::default(),
        );
        let compiled = compile_generated_residual_affine_when_bad_pullback_gate_table(
            &context, &ready, limits,
        )
        .unwrap();
        let certificate = compiled.certificate();
        assert!(!certificate.events().is_empty());
        assert_eq!(
            certificate.stats().boundary_values(),
            certificate.events().len()
        );
        certificate.replay(&context, &ready).unwrap();

        let hazard_census = census_hazards(&ready, limits).unwrap();
        let event_capacity_envelope = capacity_envelope(hazard_census.boundary_values).unwrap();
        let pre_plan_retained_envelope = checked_add(
            "generated affine pullback/gate retained bytes",
            size_of::<GeneratedResidualAffineWhenBadPullbackGateCertificate>(),
            checked_mul(
                "generated affine pullback/gate retained bytes",
                event_capacity_envelope,
                size_of::<GeneratedResidualAffineWhenBadPullbackGateEvent>(),
            )
            .unwrap(),
        )
        .unwrap();
        let integer_system = ready.input().target_branch().integer_system_arc().unwrap();
        let plan_preflight = preflight_composition_plan_retained_shape(
            &context,
            integer_system,
            limits.composition_plan,
        )
        .unwrap();
        let plan_shape = plan_preflight.shape;
        assert!(plan_shape.free_positions > 0);
        assert!(plan_shape.linear_support_entries > 0);
        assert_eq!(
            plan_shape.free_positions + plan_shape.nonfree_positions,
            context.index_count()
        );
        let plan_retained_envelope = composition_plan_retained_byte_census(plan_preflight)
            .unwrap()
            .total()
            .unwrap();
        let exact_preconstruction_admission = admit_retained_envelope_before_plan(
            pre_plan_retained_envelope,
            plan_retained_envelope,
            context.fingerprint().len(),
            usize::MAX,
        )
        .unwrap();
        admit_retained_envelope_before_plan(
            pre_plan_retained_envelope,
            plan_retained_envelope,
            context.fingerprint().len(),
            exact_preconstruction_admission,
        )
        .unwrap();

        let mut one_below = limits;
        one_below.max_retained_bytes = exact_preconstruction_admission - 1;
        assert!(matches!(
            compile_generated_residual_affine_when_bad_pullback_gate_table(
                &context, &ready, one_below,
            ),
            Err(
                GeneratedResidualAffineWhenBadPullbackGateError::ResourceLimit {
                    resource: "generated affine pullback/gate retained bytes",
                    requested,
                    limit,
                }
            ) if requested == exact_preconstruction_admission
                && limit + 1 == exact_preconstruction_admission
        ));

        COMPOSITION_PLAN_PREFLIGHT_INTEGER_SCANS.with(|scans| scans.set(0));
        let mut hostile_geometry = limits;
        hostile_geometry
            .composition_plan
            .max_geometry_entries_inspected = 0;
        assert!(matches!(
            compile_generated_residual_affine_when_bad_pullback_gate_table(
                &context,
                &ready,
                hostile_geometry,
            ),
            Err(GeneratedResidualAffineWhenBadPullbackGateError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource: "affine geometry entries inspected",
                    requested,
                    limit: 0,
                }
            )) if requested > 0
        ));
        COMPOSITION_PLAN_PREFLIGHT_INTEGER_SCANS.with(|scans| assert_eq!(scans.get(), 0));
    }
}
