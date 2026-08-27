//! Replayable coordinate-affine terminals over direct formula-residual paths.
//!
//! This owner deliberately starts from the backend-neutral direct residual
//! path.  It recognizes only exact coordinate loci, constructs their canonical
//! affine cylinder, and delegates every guard substitution to Symbolica's
//! compact affine-composition plan.  No Boolean-cover, legacy branch-system,
//! or integer-system certificate is synthesized at this boundary.

use std::fmt;
use std::mem::size_of;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::coordinate_equality_loci::{
    CoordinateEqualityLocusError, CoordinateEqualityLocusLimits,
    recognize_coordinate_locus_for_pruning,
};
use crate::exact_identity::{
    ExactIdentityError, ExactIdentityLimits, ExactIdentityPayload, ExactIdentityWriter,
    ExactStructuralIdentity, encode_exact_identity,
};
use crate::parametric_coefficient::{
    PreparedResidualAffineCompactGuardComposition, ResidualAffineCompactCompositionPlan,
    ResidualAffineCompactCompositionPlanLimits, ResidualAffineCompactMapView,
    ResidualUnitAffineCompositionError, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats,
};
use crate::parametric_sector_formula_ir::NormalizedBadLiteralPolarity;
use crate::parametric_sector_formula_residual::{
    ParametricSectorFormulaResidualDecision, ParametricSectorFormulaResidualError,
    ParametricSectorFormulaResidualKind, ParametricSectorFormulaResidualPathCertificate,
    ParametricSectorFormulaResidualPolarity,
};
use crate::{
    GuardOrigin, IntegralFamily, IntegralOrderingPolicy, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial, SectorMask,
    SectorOrthantSide,
};

pub(crate) const PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_V1_SCHEMA: &str =
    "rustred-parametric-sector-formula-affine-terminal-v1";
pub(crate) const PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_STABLE_VALUE_IDENTITY_V1_SCHEMA: &str =
    "rustred-parametric-sector-formula-affine-terminal-stable-value-identity-v1";

/// Independent resource envelope for one direct coordinate-affine terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineTerminalLimits {
    pub(crate) coordinate_loci: CoordinateEqualityLocusLimits,
    pub(crate) compact_plan: ResidualAffineCompactCompositionPlanLimits,
    pub(crate) guard_composition: ResidualUnitAffinePolynomialCompositionLimits,
    pub(crate) max_family_fingerprint_bytes: usize,
    pub(crate) max_context_fingerprint_bytes: usize,
    pub(crate) max_ambient_arity: usize,
    pub(crate) max_decisions: usize,
    pub(crate) max_recognition_entries: usize,
    pub(crate) max_unsupported_reasons: usize,
    pub(crate) max_excluded_coordinates: usize,
    pub(crate) max_excluded_coordinate_bytes: usize,
    pub(crate) max_coordinate_conflict_comparisons: usize,
    pub(crate) max_fixed_coordinates: usize,
    pub(crate) max_free_positions: usize,
    pub(crate) max_compact_matrix_entries: usize,
    pub(crate) max_cylinder_geometry_capacity_bytes: usize,
    pub(crate) max_guard_entries: usize,
    pub(crate) max_prepared_guard_token_bytes: usize,
    pub(crate) max_retained_guard_entry_bytes: usize,
    pub(crate) max_total_guard_origin_retained_bytes: usize,
    pub(crate) max_total_guard_source_terms: usize,
    pub(crate) max_total_guard_source_exponent_entries: usize,
    pub(crate) max_total_guard_expanded_contribution_bound: usize,
    pub(crate) max_total_guard_output_term_bound: usize,
    pub(crate) max_total_guard_output_terms: usize,
    pub(crate) max_total_guard_output_exponent_entry_bound: usize,
    pub(crate) max_total_guard_output_exponent_entries: usize,
    pub(crate) max_total_guard_power_calls: usize,
    pub(crate) max_total_guard_native_power_heap_pairs: usize,
    pub(crate) max_total_guard_multiplication_term_pairs: usize,
    pub(crate) max_total_guard_addition_term_visits: usize,
    pub(crate) max_total_guard_native_integer_bit_work_bound: usize,
    pub(crate) max_total_guard_integer_bit_work_bound: usize,
}

impl Default for ParametricSectorFormulaAffineTerminalLimits {
    fn default() -> Self {
        Self {
            coordinate_loci: CoordinateEqualityLocusLimits::default(),
            compact_plan: ResidualAffineCompactCompositionPlanLimits::default(),
            guard_composition: ResidualUnitAffinePolynomialCompositionLimits::default(),
            max_family_fingerprint_bytes: 1024 * 1024,
            max_context_fingerprint_bytes: 1024 * 1024,
            max_ambient_arity: 16_000_000,
            max_decisions: 16_000_000,
            max_recognition_entries: 16_000_000,
            max_unsupported_reasons: 16_000_000,
            max_excluded_coordinates: 16_000_000,
            max_excluded_coordinate_bytes: 1024 * 1024 * 1024,
            max_coordinate_conflict_comparisons: 1_000_000_000,
            max_fixed_coordinates: 16_000_000,
            max_free_positions: 16_000_000,
            max_compact_matrix_entries: 268_435_456,
            max_cylinder_geometry_capacity_bytes: 1024 * 1024 * 1024,
            max_guard_entries: 16_000_000,
            max_prepared_guard_token_bytes: 1024 * 1024 * 1024,
            max_retained_guard_entry_bytes: 1024 * 1024 * 1024,
            max_total_guard_origin_retained_bytes: 1024 * 1024 * 1024,
            max_total_guard_source_terms: 512_000_000,
            max_total_guard_source_exponent_entries: 16_000_000_000,
            max_total_guard_expanded_contribution_bound: 512_000_000,
            max_total_guard_output_term_bound: 512_000_000,
            max_total_guard_output_terms: 512_000_000,
            max_total_guard_output_exponent_entry_bound: 32_000_000_000,
            max_total_guard_output_exponent_entries: 16_000_000_000,
            max_total_guard_power_calls: 16_000_000_000,
            max_total_guard_native_power_heap_pairs: 32_000_000_000,
            max_total_guard_multiplication_term_pairs: 32_000_000_000,
            max_total_guard_addition_term_visits: 32_000_000_000,
            max_total_guard_native_integer_bit_work_bound: 16_000_000_000_000_000,
            max_total_guard_integer_bit_work_bound: 16_000_000_000_000_000,
        }
    }
}

/// One exact result of applying the coordinate-locus recognizer to a path
/// decision. `None` is retained deliberately: unresolved nonzero guards can
/// still be mapped, while an unresolved equality makes the terminal
/// unsupported.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineCoordinateRecognition {
    decision_ordinal: usize,
    decision: ParametricSectorFormulaResidualDecision,
    coordinate: Option<(usize, i64)>,
}

impl ParametricSectorFormulaAffineCoordinateRecognition {
    pub(crate) const fn decision_ordinal(self) -> usize {
        self.decision_ordinal
    }

    pub(crate) const fn decision(self) -> ParametricSectorFormulaResidualDecision {
        self.decision
    }

    pub(crate) const fn coordinate(self) -> Option<(usize, i64)> {
        self.coordinate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaAffineUnsupportedReason {
    SourceTerminalUnsupported,
    UnrecognizedEqualZero {
        decision_ordinal: usize,
        decision: ParametricSectorFormulaResidualDecision,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaAffineEmptyReason {
    OrthantViolation {
        decision_ordinal: usize,
        decision: ParametricSectorFormulaResidualDecision,
        index: usize,
        value: i64,
        side: SectorOrthantSide,
    },
    ConflictingFixedValues {
        first_decision_ordinal: usize,
        first_decision: ParametricSectorFormulaResidualDecision,
        second_decision_ordinal: usize,
        second_decision: ParametricSectorFormulaResidualDecision,
        index: usize,
        first_value: i64,
        second_value: i64,
    },
    EqualityNonzeroCoordinateConflict {
        equality_decision_ordinal: usize,
        equality_decision: ParametricSectorFormulaResidualDecision,
        nonzero_decision_ordinal: usize,
        nonzero_decision: ParametricSectorFormulaResidualDecision,
        index: usize,
        value: i64,
    },
    MappedNonzeroGuardContradiction {
        guard_entry_ordinal: usize,
        decision_ordinal: usize,
        decision: ParametricSectorFormulaResidualDecision,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaAffineTerminalOutcome {
    ProvedEmpty(ParametricSectorFormulaAffineEmptyReason),
    Unsupported,
    Actionable,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaAffineGuardClass {
    Contradiction(ParametricPolynomial),
    DischargedNonzeroIntegerConstant(ParametricPolynomial),
    BaseAssumption(ParametricNonZeroCondition),
    FreeIndexDependent(ParametricNonZeroCondition),
}

impl ParametricSectorFormulaAffineGuardClass {
    pub(crate) fn mapped_polynomial(&self) -> &ParametricPolynomial {
        match self {
            Self::Contradiction(polynomial)
            | Self::DischargedNonzeroIntegerConstant(polynomial) => polynomial,
            Self::BaseAssumption(condition) | Self::FreeIndexDependent(condition) => {
                condition.polynomial()
            }
        }
    }

    pub(crate) fn condition(&self) -> Option<&ParametricNonZeroCondition> {
        match self {
            Self::BaseAssumption(condition) | Self::FreeIndexDependent(condition) => {
                Some(condition)
            }
            Self::Contradiction(_) | Self::DischargedNonzeroIntegerConstant(_) => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineGuardEntry {
    decision_ordinal: usize,
    decision: ParametricSectorFormulaResidualDecision,
    class: ParametricSectorFormulaAffineGuardClass,
    composition_stats: ResidualUnitAffinePolynomialCompositionStats,
}

impl ParametricSectorFormulaAffineGuardEntry {
    pub(crate) const fn decision_ordinal(&self) -> usize {
        self.decision_ordinal
    }

    pub(crate) const fn decision(&self) -> ParametricSectorFormulaResidualDecision {
        self.decision
    }

    pub(crate) fn mapped_polynomial(&self) -> &ParametricPolynomial {
        self.class.mapped_polynomial()
    }

    pub(crate) const fn class(&self) -> &ParametricSectorFormulaAffineGuardClass {
        &self.class
    }

    pub(crate) const fn composition_stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.composition_stats
    }
}

/// Canonical cylinder `n = c + B t` for the recognized coordinate equalities.
/// `B` is row-major with shape `ambient_arity * free_positions.len()`.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineCylinderGeometry {
    ambient_arity: usize,
    constants: Vec<Integer>,
    free_positions: Vec<usize>,
    compact_linear_coefficients: Vec<Integer>,
}

impl ParametricSectorFormulaAffineCylinderGeometry {
    pub(crate) const fn ambient_arity(&self) -> usize {
        self.ambient_arity
    }

    pub(crate) fn constants(&self) -> &[Integer] {
        &self.constants
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    pub(crate) fn compact_linear_coefficients(&self) -> &[Integer] {
        &self.compact_linear_coefficients
    }

    fn view<'geometry>(
        &'geometry self,
        context_fingerprint: &'geometry str,
    ) -> ResidualAffineCompactMapView<'geometry> {
        ResidualAffineCompactMapView::new(
            context_fingerprint,
            self.ambient_arity,
            &self.constants,
            &self.free_positions,
            &self.compact_linear_coefficients,
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineGuardCompositionStats {
    source_terms: usize,
    source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    output_term_bound: usize,
    output_terms: usize,
    output_exponent_entry_bound: usize,
    output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pairs: usize,
    multiplication_term_pairs: usize,
    addition_term_visits: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work_bound: usize,
    integer_bit_work_bound: usize,
}

macro_rules! affine_guard_composition_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricSectorFormulaAffineGuardCompositionStats {
    affine_guard_composition_stats_getters!(
        source_terms,
        source_exponent_entries,
        expanded_contribution_bound,
        output_term_bound,
        output_terms,
        output_exponent_entry_bound,
        output_exponent_entries,
        power_calls,
        native_power_heap_pairs,
        multiplication_term_pairs,
        addition_term_visits,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bit_bound,
        native_integer_bit_work_bound,
        integer_bit_work_bound,
    );
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricSectorFormulaAffineTerminalStats {
    decisions: usize,
    coordinate_recognitions: usize,
    recognized_coordinate_loci: usize,
    unrecognized_equal_zero_loci: usize,
    excluded_coordinates: usize,
    excluded_coordinate_capacity_bytes: usize,
    coordinate_conflict_comparisons: usize,
    fixed_coordinates: usize,
    free_positions: usize,
    compact_matrix_entries: usize,
    cylinder_geometry_byte_envelope: usize,
    cylinder_geometry_capacity_bytes: usize,
    unsupported_reasons: usize,
    prepared_guard_token_byte_envelope: usize,
    prepared_guard_token_capacity_bytes: usize,
    retained_guard_entry_capacity_bytes: usize,
    guard_origin_retained_byte_envelope: usize,
    total_guard_origin_retained_bytes: usize,
    guard_entries: usize,
    guard_contradictions: usize,
    discharged_nonzero_integer_constants: usize,
    base_assumptions: usize,
    free_index_dependent_conditions: usize,
    guard_preflight: ParametricSectorFormulaAffineGuardCompositionStats,
    guard_execution: ParametricSectorFormulaAffineGuardCompositionStats,
}

macro_rules! affine_terminal_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricSectorFormulaAffineTerminalStats {
    affine_terminal_stats_getters!(
        decisions,
        coordinate_recognitions,
        recognized_coordinate_loci,
        unrecognized_equal_zero_loci,
        excluded_coordinates,
        excluded_coordinate_capacity_bytes,
        coordinate_conflict_comparisons,
        fixed_coordinates,
        free_positions,
        compact_matrix_entries,
        cylinder_geometry_byte_envelope,
        cylinder_geometry_capacity_bytes,
        unsupported_reasons,
        prepared_guard_token_byte_envelope,
        prepared_guard_token_capacity_bytes,
        retained_guard_entry_capacity_bytes,
        guard_origin_retained_byte_envelope,
        total_guard_origin_retained_bytes,
        guard_entries,
        guard_contradictions,
        discharged_nonzero_integer_constants,
        base_assumptions,
        free_index_dependent_conditions,
    );

    pub(crate) const fn guard_preflight(
        self,
    ) -> ParametricSectorFormulaAffineGuardCompositionStats {
        self.guard_preflight
    }

    pub(crate) const fn guard_execution(
        self,
    ) -> ParametricSectorFormulaAffineGuardCompositionStats {
        self.guard_execution
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ParametricSectorFormulaAffineTerminalError {
    ReplayMismatch,
    SourceShapeMismatch,
    StructuralLocusOutOfRange {
        decision_ordinal: usize,
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
    ResidualPath(ParametricSectorFormulaResidualError),
    CoordinateLocus(CoordinateEqualityLocusError),
    Composition(ResidualUnitAffineCompositionError),
    ParametricCoefficient(ParametricCoefficientError),
}

impl fmt::Display for ParametricSectorFormulaAffineTerminalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "parametric sector formula affine-terminal error: {self:?}"
        )
    }
}

impl std::error::Error for ParametricSectorFormulaAffineTerminalError {}

impl From<ParametricSectorFormulaResidualError> for ParametricSectorFormulaAffineTerminalError {
    fn from(value: ParametricSectorFormulaResidualError) -> Self {
        Self::ResidualPath(value)
    }
}

impl From<CoordinateEqualityLocusError> for ParametricSectorFormulaAffineTerminalError {
    fn from(value: CoordinateEqualityLocusError) -> Self {
        Self::CoordinateLocus(value)
    }
}

impl From<ResidualUnitAffineCompositionError> for ParametricSectorFormulaAffineTerminalError {
    fn from(value: ResidualUnitAffineCompositionError) -> Self {
        Self::Composition(value)
    }
}

impl From<ParametricCoefficientError> for ParametricSectorFormulaAffineTerminalError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::ParametricCoefficient(value)
    }
}

/// Standalone proof owner for one exact direct residual path.
pub(crate) struct ParametricSectorFormulaAffineTerminalCertificate {
    schema: &'static str,
    path: Arc<ParametricSectorFormulaResidualPathCertificate>,
    recognitions: Vec<ParametricSectorFormulaAffineCoordinateRecognition>,
    unsupported_reasons: Vec<ParametricSectorFormulaAffineUnsupportedReason>,
    geometry: Option<ParametricSectorFormulaAffineCylinderGeometry>,
    compact_plan: Option<ResidualAffineCompactCompositionPlan>,
    guards: Vec<ParametricSectorFormulaAffineGuardEntry>,
    outcome: ParametricSectorFormulaAffineTerminalOutcome,
    limits: ParametricSectorFormulaAffineTerminalLimits,
    stats: ParametricSectorFormulaAffineTerminalStats,
}

impl fmt::Debug for ParametricSectorFormulaAffineTerminalCertificate {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricSectorFormulaAffineTerminalCertificate")
            .field("schema", &self.schema)
            .field("terminal_kind", &self.path.terminal_kind())
            .field("ordering_policy", &self.ordering_policy())
            .field("recognitions", &self.recognitions)
            .field("unsupported_reasons", &self.unsupported_reasons)
            .field("geometry", &self.geometry)
            .field("guards", &self.guards)
            .field("outcome", &self.outcome)
            .field("stats", &self.stats)
            .field("path", &"<exact shared direct residual path>")
            .finish()
    }
}

impl ParametricSectorFormulaAffineTerminalCertificate {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn path_arc(&self) -> &Arc<ParametricSectorFormulaResidualPathCertificate> {
        &self.path
    }

    pub(crate) fn same_path_allocation(
        &self,
        path: &Arc<ParametricSectorFormulaResidualPathCertificate>,
    ) -> bool {
        Arc::ptr_eq(&self.path, path)
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.path.source_arc().family_fingerprint()
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.path.source_arc().context_fingerprint()
    }

    pub(crate) fn sector(&self) -> &SectorMask {
        self.path.source_arc().sector()
    }

    pub(crate) fn ordering_policy(&self) -> IntegralOrderingPolicy {
        self.path.source_arc().ordering_policy()
    }

    pub(crate) fn terminal_kind(&self) -> ParametricSectorFormulaResidualKind {
        self.path.terminal_kind()
    }

    pub(crate) fn recognitions(&self) -> &[ParametricSectorFormulaAffineCoordinateRecognition] {
        &self.recognitions
    }

    pub(crate) fn unsupported_reasons(&self) -> &[ParametricSectorFormulaAffineUnsupportedReason] {
        &self.unsupported_reasons
    }

    pub(crate) const fn geometry(&self) -> Option<&ParametricSectorFormulaAffineCylinderGeometry> {
        self.geometry.as_ref()
    }

    pub(crate) fn guards(&self) -> &[ParametricSectorFormulaAffineGuardEntry] {
        &self.guards
    }

    pub(crate) const fn outcome(&self) -> ParametricSectorFormulaAffineTerminalOutcome {
        self.outcome
    }

    pub(crate) const fn limits(&self) -> ParametricSectorFormulaAffineTerminalLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ParametricSectorFormulaAffineTerminalStats {
        self.stats
    }

    /// Allocation-independent durable identity for the entire direct source
    /// chain and this terminal's retained proof. Allocation ancestry remains
    /// a separate replay invariant and is deliberately not serialized.
    pub(crate) fn encode_durable_identity(
        &self,
        limits: ExactIdentityLimits,
    ) -> Result<ExactStructuralIdentity, ExactIdentityError> {
        encode_exact_identity(self, limits)
    }

    pub(crate) const fn durable_identity_schema(&self) -> &'static str {
        PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_STABLE_VALUE_IDENTITY_V1_SCHEMA
    }

    fn write_stable_value_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
        tag: &str,
    ) -> Result<(), ExactIdentityError> {
        writer.begin_record(tag, 11)?;
        writer.string(
            "identity_schema",
            PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_STABLE_VALUE_IDENTITY_V1_SCHEMA,
        )?;
        writer.string("certificate_schema", self.schema)?;
        self.path.write_stable_value_identity(writer, "path")?;
        writer.begin_sequence("recognitions", self.recognitions.len())?;
        for recognition in &self.recognitions {
            write_coordinate_recognition_identity(writer, "recognition", *recognition)?;
        }
        writer.end_sequence()?;
        writer.begin_sequence("unsupported_reasons", self.unsupported_reasons.len())?;
        for reason in &self.unsupported_reasons {
            write_unsupported_reason_identity(writer, "reason", *reason)?;
        }
        writer.end_sequence()?;
        write_geometry_identity(writer, "geometry", self.geometry.as_ref())?;
        write_compact_plan_option_identity(writer, "compact_plan", self.compact_plan.as_ref())?;
        writer.begin_sequence("guards", self.guards.len())?;
        for (ordinal, guard) in self.guards.iter().enumerate() {
            write_guard_entry_identity(writer, "guard", ordinal, guard)?;
        }
        writer.end_sequence()?;
        write_terminal_outcome_identity(writer, "outcome", self.outcome)?;
        write_terminal_limits_identity(writer, "limits", self.limits)?;
        write_terminal_stats_identity(writer, "stats", self.stats)?;
        writer.end_record()
    }

    /// Replay the exact path allocation and rebuild the complete terminal.
    pub(crate) fn replay(
        &self,
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
        if self.schema != PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_V1_SCHEMA {
            return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
        }
        self.path.replay(family, context)?;
        if let (Some(geometry), Some(plan)) = (&self.geometry, &self.compact_plan) {
            plan.replay(context, geometry.view(context.fingerprint()))?;
        } else if self.geometry.is_some() != self.compact_plan.is_some() {
            return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
        }
        let rebuilt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compile_replayed_path(context, Arc::clone(&self.path), self.limits)
        }))
        .map_err(
            |_| ParametricSectorFormulaAffineTerminalError::SymbolicaPanic {
                stage: "direct formula coordinate-affine terminal replay",
            },
        )??;
        if self.payload_eq(&rebuilt) {
            Ok(())
        } else {
            Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch)
        }
    }

    fn payload_eq(&self, other: &Self) -> bool {
        self.schema == other.schema
            && Arc::ptr_eq(&self.path, &other.path)
            && self.recognitions == other.recognitions
            && self.unsupported_reasons == other.unsupported_reasons
            && self.geometry == other.geometry
            && self.guards == other.guards
            && self.outcome == other.outcome
            && self.limits == other.limits
            && self.stats == other.stats
            && match (&self.compact_plan, &other.compact_plan) {
                (Some(left), Some(right)) => left.manifest() == right.manifest(),
                (None, None) => true,
                _ => false,
            }
    }
}

impl ExactIdentityPayload for ParametricSectorFormulaAffineTerminalCertificate {
    const SCHEMA: &'static str =
        PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_STABLE_VALUE_IDENTITY_V1_SCHEMA;

    fn write_exact_identity(
        &self,
        writer: &mut ExactIdentityWriter<'_>,
    ) -> Result<(), ExactIdentityError> {
        self.write_stable_value_identity(writer, "terminal")
    }
}

fn write_formula_residual_decision_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    decision: ParametricSectorFormulaResidualDecision,
) -> Result<(), ExactIdentityError> {
    let split = decision.split();
    writer.begin_record(tag, 6)?;
    writer.usize("source_attempt_ordinal", split.source_attempt_ordinal())?;
    writer.usize("clause_ordinal", split.clause_ordinal())?;
    writer.usize("literal_position", usize::from(split.literal_position()))?;
    writer.usize("structural_locus_ordinal", split.structural_locus_ordinal())?;
    writer.variant(
        "bad_literal_polarity",
        match split.bad_literal_polarity() {
            NormalizedBadLiteralPolarity::EqualZero => "EqualZero",
            NormalizedBadLiteralPolarity::NonZero => "NonZero",
        },
    )?;
    writer.variant(
        "branch_polarity",
        match decision.polarity() {
            ParametricSectorFormulaResidualPolarity::NonZero => "NonZero",
            ParametricSectorFormulaResidualPolarity::EqualZero => "EqualZero",
        },
    )?;
    writer.end_record()
}

fn write_coordinate_recognition_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    recognition: ParametricSectorFormulaAffineCoordinateRecognition,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.usize("decision_ordinal", recognition.decision_ordinal())?;
    write_formula_residual_decision_identity(writer, "decision", recognition.decision())?;
    writer.begin_record("coordinate", 2)?;
    match recognition.coordinate() {
        Some((index, value)) => {
            writer.variant("variant", "Some")?;
            writer.begin_record("fields", 2)?;
            writer.usize("index", index)?;
            writer.signed_i64("value", value)?;
            writer.end_record()?;
        }
        None => {
            writer.variant("variant", "None")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()?;
    writer.end_record()
}

fn write_unsupported_reason_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    reason: ParametricSectorFormulaAffineUnsupportedReason,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match reason {
        ParametricSectorFormulaAffineUnsupportedReason::SourceTerminalUnsupported => {
            writer.variant("variant", "SourceTerminalUnsupported")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
        ParametricSectorFormulaAffineUnsupportedReason::UnrecognizedEqualZero {
            decision_ordinal,
            decision,
        } => {
            writer.variant("variant", "UnrecognizedEqualZero")?;
            writer.begin_record("fields", 2)?;
            writer.usize("decision_ordinal", decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "decision", decision)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_geometry_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    geometry: Option<&ParametricSectorFormulaAffineCylinderGeometry>,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match geometry {
        Some(geometry) => {
            writer.variant("variant", "Some")?;
            writer.begin_record("fields", 4)?;
            writer.usize("ambient_arity", geometry.ambient_arity())?;
            writer.begin_sequence("constants", geometry.constants().len())?;
            for constant in geometry.constants() {
                writer.integer("constant", constant)?;
            }
            writer.end_sequence()?;
            writer.begin_sequence("free_positions", geometry.free_positions().len())?;
            for &position in geometry.free_positions() {
                writer.usize("position", position)?;
            }
            writer.end_sequence()?;
            writer.begin_sequence(
                "compact_linear_coefficients",
                geometry.compact_linear_coefficients().len(),
            )?;
            for coefficient in geometry.compact_linear_coefficients() {
                writer.integer("coefficient", coefficient)?;
            }
            writer.end_sequence()?;
            writer.end_record()?;
        }
        None => {
            writer.variant("variant", "None")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_compact_plan_option_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    plan: Option<&ResidualAffineCompactCompositionPlan>,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match plan {
        Some(plan) => {
            writer.variant("variant", "Some")?;
            plan.write_stable_value_identity(writer, "value")?;
        }
        None => {
            writer.variant("variant", "None")?;
            writer.begin_record("value", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_condition_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    condition: Option<&ParametricNonZeroCondition>,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match condition {
        Some(condition) => {
            writer.variant("variant", "Some")?;
            writer.begin_record("fields", 2)?;
            writer.polynomial("polynomial", condition.polynomial().raw())?;
            writer.begin_sequence("origins", condition.origins().len())?;
            for origin in condition.origins() {
                writer.guard_origin("origin", origin)?;
            }
            writer.end_sequence()?;
            writer.end_record()?;
        }
        None => {
            writer.variant("variant", "None")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_guard_class_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    class: &ParametricSectorFormulaAffineGuardClass,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 3)?;
    writer.variant(
        "variant",
        match class {
            ParametricSectorFormulaAffineGuardClass::Contradiction(_) => "Contradiction",
            ParametricSectorFormulaAffineGuardClass::DischargedNonzeroIntegerConstant(_) => {
                "DischargedNonzeroIntegerConstant"
            }
            ParametricSectorFormulaAffineGuardClass::BaseAssumption(_) => "BaseAssumption",
            ParametricSectorFormulaAffineGuardClass::FreeIndexDependent(_) => "FreeIndexDependent",
        },
    )?;
    writer.polynomial("mapped_polynomial", class.mapped_polynomial().raw())?;
    write_condition_identity(writer, "condition", class.condition())?;
    writer.end_record()
}

fn write_guard_entry_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    ordinal: usize,
    guard: &ParametricSectorFormulaAffineGuardEntry,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 5)?;
    writer.usize("ordinal", ordinal)?;
    writer.usize("decision_ordinal", guard.decision_ordinal())?;
    write_formula_residual_decision_identity(writer, "decision", guard.decision())?;
    write_guard_class_identity(writer, "class", guard.class())?;
    crate::parametric_coefficient::write_residual_unit_affine_polynomial_composition_stats_identity(
        writer,
        "composition_stats",
        guard.composition_stats(),
    )?;
    writer.end_record()
}

fn write_terminal_outcome_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    outcome: ParametricSectorFormulaAffineTerminalOutcome,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match outcome {
        ParametricSectorFormulaAffineTerminalOutcome::ProvedEmpty(reason) => {
            writer.variant("variant", "ProvedEmpty")?;
            write_empty_reason_identity(writer, "fields", reason)?;
        }
        ParametricSectorFormulaAffineTerminalOutcome::Unsupported => {
            writer.variant("variant", "Unsupported")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
        ParametricSectorFormulaAffineTerminalOutcome::Actionable => {
            writer.variant("variant", "Actionable")?;
            writer.begin_record("fields", 0)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_empty_reason_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    reason: ParametricSectorFormulaAffineEmptyReason,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 2)?;
    match reason {
        ParametricSectorFormulaAffineEmptyReason::OrthantViolation {
            decision_ordinal,
            decision,
            index,
            value,
            side,
        } => {
            writer.variant("variant", "OrthantViolation")?;
            writer.begin_record("fields", 5)?;
            writer.usize("decision_ordinal", decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "decision", decision)?;
            writer.usize("index", index)?;
            writer.signed_i64("value", value)?;
            writer.variant(
                "side",
                match side {
                    SectorOrthantSide::AtLeastOne => "AtLeastOne",
                    SectorOrthantSide::AtMostZero => "AtMostZero",
                },
            )?;
            writer.end_record()?;
        }
        ParametricSectorFormulaAffineEmptyReason::ConflictingFixedValues {
            first_decision_ordinal,
            first_decision,
            second_decision_ordinal,
            second_decision,
            index,
            first_value,
            second_value,
        } => {
            writer.variant("variant", "ConflictingFixedValues")?;
            writer.begin_record("fields", 7)?;
            writer.usize("first_decision_ordinal", first_decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "first_decision", first_decision)?;
            writer.usize("second_decision_ordinal", second_decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "second_decision", second_decision)?;
            writer.usize("index", index)?;
            writer.signed_i64("first_value", first_value)?;
            writer.signed_i64("second_value", second_value)?;
            writer.end_record()?;
        }
        ParametricSectorFormulaAffineEmptyReason::EqualityNonzeroCoordinateConflict {
            equality_decision_ordinal,
            equality_decision,
            nonzero_decision_ordinal,
            nonzero_decision,
            index,
            value,
        } => {
            writer.variant("variant", "EqualityNonzeroCoordinateConflict")?;
            writer.begin_record("fields", 6)?;
            writer.usize("equality_decision_ordinal", equality_decision_ordinal)?;
            write_formula_residual_decision_identity(
                writer,
                "equality_decision",
                equality_decision,
            )?;
            writer.usize("nonzero_decision_ordinal", nonzero_decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "nonzero_decision", nonzero_decision)?;
            writer.usize("index", index)?;
            writer.signed_i64("value", value)?;
            writer.end_record()?;
        }
        ParametricSectorFormulaAffineEmptyReason::MappedNonzeroGuardContradiction {
            guard_entry_ordinal,
            decision_ordinal,
            decision,
        } => {
            writer.variant("variant", "MappedNonzeroGuardContradiction")?;
            writer.begin_record("fields", 3)?;
            writer.usize("guard_entry_ordinal", guard_entry_ordinal)?;
            writer.usize("decision_ordinal", decision_ordinal)?;
            write_formula_residual_decision_identity(writer, "decision", decision)?;
            writer.end_record()?;
        }
    }
    writer.end_record()
}

fn write_terminal_limits_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    limits: ParametricSectorFormulaAffineTerminalLimits,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 4)?;
    crate::parametric_sector_coverage::write_coordinate_locus_limits_identity(
        writer,
        "coordinate_loci",
        limits.coordinate_loci,
    )?;
    crate::parametric_coefficient::write_compact_plan_limits_identity(
        writer,
        "compact_plan",
        limits.compact_plan,
    )?;
    crate::parametric_coefficient::write_residual_unit_affine_polynomial_composition_limits_identity(
        writer,
        "guard_composition",
        limits.guard_composition,
    )?;
    writer.begin_record("scalar_limits", 30)?;
    writer.usize(
        "max_family_fingerprint_bytes",
        limits.max_family_fingerprint_bytes,
    )?;
    writer.usize(
        "max_context_fingerprint_bytes",
        limits.max_context_fingerprint_bytes,
    )?;
    writer.usize("max_ambient_arity", limits.max_ambient_arity)?;
    writer.usize("max_decisions", limits.max_decisions)?;
    writer.usize("max_recognition_entries", limits.max_recognition_entries)?;
    writer.usize("max_unsupported_reasons", limits.max_unsupported_reasons)?;
    writer.usize("max_excluded_coordinates", limits.max_excluded_coordinates)?;
    writer.usize(
        "max_excluded_coordinate_bytes",
        limits.max_excluded_coordinate_bytes,
    )?;
    writer.usize(
        "max_coordinate_conflict_comparisons",
        limits.max_coordinate_conflict_comparisons,
    )?;
    writer.usize("max_fixed_coordinates", limits.max_fixed_coordinates)?;
    writer.usize("max_free_positions", limits.max_free_positions)?;
    writer.usize(
        "max_compact_matrix_entries",
        limits.max_compact_matrix_entries,
    )?;
    writer.usize(
        "max_cylinder_geometry_capacity_bytes",
        limits.max_cylinder_geometry_capacity_bytes,
    )?;
    writer.usize("max_guard_entries", limits.max_guard_entries)?;
    writer.usize(
        "max_prepared_guard_token_bytes",
        limits.max_prepared_guard_token_bytes,
    )?;
    writer.usize(
        "max_retained_guard_entry_bytes",
        limits.max_retained_guard_entry_bytes,
    )?;
    writer.usize(
        "max_total_guard_origin_retained_bytes",
        limits.max_total_guard_origin_retained_bytes,
    )?;
    writer.usize(
        "max_total_guard_source_terms",
        limits.max_total_guard_source_terms,
    )?;
    writer.usize(
        "max_total_guard_source_exponent_entries",
        limits.max_total_guard_source_exponent_entries,
    )?;
    writer.usize(
        "max_total_guard_expanded_contribution_bound",
        limits.max_total_guard_expanded_contribution_bound,
    )?;
    writer.usize(
        "max_total_guard_output_term_bound",
        limits.max_total_guard_output_term_bound,
    )?;
    writer.usize(
        "max_total_guard_output_terms",
        limits.max_total_guard_output_terms,
    )?;
    writer.usize(
        "max_total_guard_output_exponent_entry_bound",
        limits.max_total_guard_output_exponent_entry_bound,
    )?;
    writer.usize(
        "max_total_guard_output_exponent_entries",
        limits.max_total_guard_output_exponent_entries,
    )?;
    writer.usize(
        "max_total_guard_power_calls",
        limits.max_total_guard_power_calls,
    )?;
    writer.usize(
        "max_total_guard_native_power_heap_pairs",
        limits.max_total_guard_native_power_heap_pairs,
    )?;
    writer.usize(
        "max_total_guard_multiplication_term_pairs",
        limits.max_total_guard_multiplication_term_pairs,
    )?;
    writer.usize(
        "max_total_guard_addition_term_visits",
        limits.max_total_guard_addition_term_visits,
    )?;
    writer.usize(
        "max_total_guard_native_integer_bit_work_bound",
        limits.max_total_guard_native_integer_bit_work_bound,
    )?;
    writer.usize(
        "max_total_guard_integer_bit_work_bound",
        limits.max_total_guard_integer_bit_work_bound,
    )?;
    writer.end_record()?;
    writer.end_record()
}

fn write_guard_composition_aggregate_stats_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    stats: ParametricSectorFormulaAffineGuardCompositionStats,
) -> Result<(), ExactIdentityError> {
    writer.begin_record(tag, 15)?;
    writer.usize("source_terms", stats.source_terms())?;
    writer.usize("source_exponent_entries", stats.source_exponent_entries())?;
    writer.usize(
        "expanded_contribution_bound",
        stats.expanded_contribution_bound(),
    )?;
    writer.usize("output_term_bound", stats.output_term_bound())?;
    writer.usize("output_terms", stats.output_terms())?;
    writer.usize(
        "output_exponent_entry_bound",
        stats.output_exponent_entry_bound(),
    )?;
    writer.usize("output_exponent_entries", stats.output_exponent_entries())?;
    writer.usize("power_calls", stats.power_calls())?;
    writer.usize("native_power_heap_pairs", stats.native_power_heap_pairs())?;
    writer.usize(
        "multiplication_term_pairs",
        stats.multiplication_term_pairs(),
    )?;
    writer.usize("addition_term_visits", stats.addition_term_visits())?;
    writer.usize(
        "largest_kronecker_exponent_bits",
        stats.largest_kronecker_exponent_bits(),
    )?;
    writer.usize(
        "largest_integer_coefficient_bit_bound",
        stats.largest_integer_coefficient_bit_bound(),
    )?;
    writer.usize(
        "native_integer_bit_work_bound",
        stats.native_integer_bit_work_bound(),
    )?;
    writer.usize("integer_bit_work_bound", stats.integer_bit_work_bound())?;
    writer.end_record()
}

fn write_terminal_stats_identity(
    writer: &mut ExactIdentityWriter<'_>,
    tag: &str,
    stats: ParametricSectorFormulaAffineTerminalStats,
) -> Result<(), ExactIdentityError> {
    // Allocator capacities and all `size_of`-derived byte envelopes are
    // intentionally excluded. They remain replay diagnostics, not stable
    // mathematical value, and can change across targets or allocators.
    writer.begin_record(tag, 17)?;
    writer.usize("decisions", stats.decisions())?;
    writer.usize("coordinate_recognitions", stats.coordinate_recognitions())?;
    writer.usize(
        "recognized_coordinate_loci",
        stats.recognized_coordinate_loci(),
    )?;
    writer.usize(
        "unrecognized_equal_zero_loci",
        stats.unrecognized_equal_zero_loci(),
    )?;
    writer.usize("excluded_coordinates", stats.excluded_coordinates())?;
    writer.usize(
        "coordinate_conflict_comparisons",
        stats.coordinate_conflict_comparisons(),
    )?;
    writer.usize("fixed_coordinates", stats.fixed_coordinates())?;
    writer.usize("free_positions", stats.free_positions())?;
    writer.usize("compact_matrix_entries", stats.compact_matrix_entries())?;
    writer.usize("unsupported_reasons", stats.unsupported_reasons())?;
    writer.usize("guard_entries", stats.guard_entries())?;
    writer.usize("guard_contradictions", stats.guard_contradictions())?;
    writer.usize(
        "discharged_nonzero_integer_constants",
        stats.discharged_nonzero_integer_constants(),
    )?;
    writer.usize("base_assumptions", stats.base_assumptions())?;
    writer.usize(
        "free_index_dependent_conditions",
        stats.free_index_dependent_conditions(),
    )?;
    write_guard_composition_aggregate_stats_identity(
        writer,
        "guard_preflight",
        stats.guard_preflight(),
    )?;
    write_guard_composition_aggregate_stats_identity(
        writer,
        "guard_execution",
        stats.guard_execution(),
    )?;
    writer.end_record()
}

#[derive(Clone, Copy, Debug)]
struct FixedCoordinate {
    value: i64,
    decision_ordinal: usize,
    decision: ParametricSectorFormulaResidualDecision,
}

#[derive(Clone, Copy, Debug)]
struct ExcludedCoordinate {
    index: usize,
    value: i64,
    decision_ordinal: usize,
    decision: ParametricSectorFormulaResidualDecision,
}

type PreparedGuard<'prepared> = (
    usize,
    ParametricSectorFormulaResidualDecision,
    PreparedResidualAffineCompactGuardComposition<'prepared>,
);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct GuardCompositionClampProvenance {
    source_terms: bool,
    source_exponent_entries: bool,
    expanded_contribution_bound: bool,
    output_term_bound: bool,
    output_terms: bool,
    output_exponent_entry_bound: bool,
    output_exponent_entries: bool,
    power_calls: bool,
    native_power_heap_pairs: bool,
    multiplication_term_pairs: bool,
    addition_term_visits: bool,
    native_integer_bit_work: bool,
    integer_bit_work: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuardCompositionCallLimits {
    effective: ResidualUnitAffinePolynomialCompositionLimits,
    clamps: GuardCompositionClampProvenance,
}

pub(crate) struct ParametricSectorFormulaAffineTerminalCompiler;

impl ParametricSectorFormulaAffineTerminalCompiler {
    pub(crate) fn compile(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        path: Arc<ParametricSectorFormulaResidualPathCertificate>,
        limits: ParametricSectorFormulaAffineTerminalLimits,
    ) -> Result<
        ParametricSectorFormulaAffineTerminalCertificate,
        ParametricSectorFormulaAffineTerminalError,
    > {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            path.replay(family, context)?;
            compile_replayed_path(context, path, limits)
        }))
        .map_err(
            |_| ParametricSectorFormulaAffineTerminalError::SymbolicaPanic {
                stage: "direct formula coordinate-affine terminal compilation",
            },
        )?
    }
}

fn compile_replayed_path(
    context: &ParametricCoefficientContext,
    path: Arc<ParametricSectorFormulaResidualPathCertificate>,
    limits: ParametricSectorFormulaAffineTerminalLimits,
) -> Result<
    ParametricSectorFormulaAffineTerminalCertificate,
    ParametricSectorFormulaAffineTerminalError,
> {
    let source = path.source_arc();
    check_limit(
        "affine-terminal family fingerprint bytes",
        source.family_fingerprint().len(),
        limits.max_family_fingerprint_bytes,
    )?;
    check_limit(
        "affine-terminal context fingerprint bytes",
        source.context_fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    let arity = context.index_count();
    if arity != source.sector().arity() {
        return Err(ParametricSectorFormulaAffineTerminalError::SourceShapeMismatch);
    }
    check_limit(
        "affine-terminal ambient arity",
        arity,
        limits.max_ambient_arity,
    )?;
    let decisions = path.decisions();
    check_limit(
        "affine-terminal decisions",
        decisions.len(),
        limits.max_decisions,
    )?;
    check_limit(
        "affine-terminal recognition entries",
        decisions.len(),
        limits.max_recognition_entries,
    )?;
    let guard_count = decisions
        .iter()
        .filter(|decision| decision.polarity() == ParametricSectorFormulaResidualPolarity::NonZero)
        .count();
    check_limit(
        "affine-terminal guard entries",
        guard_count,
        limits.max_guard_entries,
    )?;
    check_limit(
        "affine-terminal excluded coordinates",
        guard_count,
        limits.max_excluded_coordinates,
    )?;
    let mut recognitions = Vec::new();
    try_reserve_exact(
        &mut recognitions,
        decisions.len(),
        "affine-terminal recognition entries",
    )?;
    let mut fixed = Vec::new();
    try_reserve_exact(&mut fixed, arity, "affine-terminal fixed-coordinate table")?;
    fixed.resize(arity, None::<FixedCoordinate>);
    let excluded_byte_envelope = checked_mul(
        "affine-terminal excluded-coordinate bytes",
        guard_count,
        size_of::<ExcludedCoordinate>(),
    )?;
    check_limit(
        "affine-terminal excluded-coordinate bytes",
        excluded_byte_envelope,
        limits.max_excluded_coordinate_bytes,
    )?;
    let mut excluded = Vec::<ExcludedCoordinate>::new();
    try_reserve_exact(
        &mut excluded,
        guard_count,
        "affine-terminal excluded coordinates",
    )?;
    let mut unsupported_reasons = Vec::new();
    let mut stats = ParametricSectorFormulaAffineTerminalStats {
        decisions: decisions.len(),
        excluded_coordinate_capacity_bytes: checked_mul(
            "affine-terminal excluded-coordinate bytes",
            excluded.capacity(),
            size_of::<ExcludedCoordinate>(),
        )?,
        ..ParametricSectorFormulaAffineTerminalStats::default()
    };
    check_limit(
        "affine-terminal excluded-coordinate bytes",
        stats.excluded_coordinate_capacity_bytes,
        limits.max_excluded_coordinate_bytes,
    )?;
    let mut first_empty = None;

    if path.terminal_kind() == ParametricSectorFormulaResidualKind::Unsupported {
        push_unsupported_reason(
            &mut unsupported_reasons,
            ParametricSectorFormulaAffineUnsupportedReason::SourceTerminalUnsupported,
            limits,
        )?;
    }

    for (decision_ordinal, &decision) in decisions.iter().enumerate() {
        let polynomial = path.structural_locus(decision_ordinal).ok_or(
            ParametricSectorFormulaAffineTerminalError::StructuralLocusOutOfRange {
                decision_ordinal,
            },
        )?;
        let coordinate =
            recognize_coordinate_locus_for_pruning(context, polynomial, limits.coordinate_loci)?;
        recognitions.push(ParametricSectorFormulaAffineCoordinateRecognition {
            decision_ordinal,
            decision,
            coordinate,
        });
        stats.coordinate_recognitions = checked_add(
            "affine-terminal coordinate recognitions",
            stats.coordinate_recognitions,
            1,
        )?;
        if coordinate.is_some() {
            stats.recognized_coordinate_loci = checked_add(
                "affine-terminal recognized coordinate loci",
                stats.recognized_coordinate_loci,
                1,
            )?;
        } else if decision.polarity() == ParametricSectorFormulaResidualPolarity::EqualZero {
            stats.unrecognized_equal_zero_loci = checked_add(
                "affine-terminal unrecognized equal-zero loci",
                stats.unrecognized_equal_zero_loci,
                1,
            )?;
            push_unsupported_reason(
                &mut unsupported_reasons,
                ParametricSectorFormulaAffineUnsupportedReason::UnrecognizedEqualZero {
                    decision_ordinal,
                    decision,
                },
                limits,
            )?;
        }
        if first_empty.is_none() {
            first_empty = apply_coordinate_decision(
                source.sector(),
                &mut fixed,
                &mut excluded,
                &mut stats,
                limits,
                decision_ordinal,
                decision,
                coordinate,
            )?;
        }
    }
    stats.excluded_coordinates = excluded.len();
    stats.unsupported_reasons = unsupported_reasons.len();

    if let Some(reason) = first_empty {
        return Ok(ParametricSectorFormulaAffineTerminalCertificate {
            schema: PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_V1_SCHEMA,
            path,
            recognitions,
            unsupported_reasons,
            geometry: None,
            compact_plan: None,
            guards: Vec::new(),
            outcome: ParametricSectorFormulaAffineTerminalOutcome::ProvedEmpty(reason),
            limits,
            stats,
        });
    }
    if !unsupported_reasons.is_empty() {
        return Ok(ParametricSectorFormulaAffineTerminalCertificate {
            schema: PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_V1_SCHEMA,
            path,
            recognitions,
            unsupported_reasons,
            geometry: None,
            compact_plan: None,
            guards: Vec::new(),
            outcome: ParametricSectorFormulaAffineTerminalOutcome::Unsupported,
            limits,
            stats,
        });
    }

    // Only an actionable path can retain mapped guard conditions.  Admit the
    // conservative per-guard origin envelope after coordinate reasoning so
    // an irrelevant condition-origin limit cannot reject an already empty or
    // unsupported path, but still before any affine plan or guard token is
    // prepared.
    let guard_origin_bytes = GuardOrigin::GeneratedAffineSealedCondition
        .retained_byte_bound()
        .ok_or(
            ParametricSectorFormulaAffineTerminalError::ResourceCountOverflow {
                resource: "affine-terminal guard origin retained bytes",
            },
        )?;
    if guard_count != 0 {
        check_limit(
            "affine-terminal per-condition guard origins",
            1,
            limits.guard_composition.max_guard_origins,
        )?;
        check_limit(
            "affine-terminal per-condition guard origin retained bytes",
            guard_origin_bytes,
            limits.guard_composition.max_guard_origin_retained_bytes,
        )?;
    }
    stats.guard_origin_retained_byte_envelope = checked_mul(
        "affine-terminal total guard origin retained bytes",
        guard_count,
        guard_origin_bytes,
    )?;
    check_limit(
        "affine-terminal total guard origin retained bytes",
        stats.guard_origin_retained_byte_envelope,
        limits.max_total_guard_origin_retained_bytes,
    )?;

    stats.prepared_guard_token_byte_envelope = checked_mul(
        "affine-terminal prepared guard token bytes",
        guard_count,
        size_of::<PreparedGuard<'_>>(),
    )?;
    check_limit(
        "affine-terminal prepared guard token bytes",
        stats.prepared_guard_token_byte_envelope,
        limits.max_prepared_guard_token_bytes,
    )?;
    let geometry = build_cylinder_geometry(&fixed, limits, &mut stats)?;
    let compact_plan = context.compile_residual_affine_compact_composition_plan(
        geometry.view(context.fingerprint()),
        limits.compact_plan,
    )?;
    compact_plan.replay(context, geometry.view(context.fingerprint()))?;

    let mut prepared = Vec::new();
    try_reserve_exact(
        &mut prepared,
        guard_count,
        "affine-terminal prepared guard entries",
    )?;
    stats.prepared_guard_token_capacity_bytes = checked_mul(
        "affine-terminal prepared guard token bytes",
        prepared.capacity(),
        size_of::<PreparedGuard<'_>>(),
    )?;
    check_limit(
        "affine-terminal prepared guard token bytes",
        stats.prepared_guard_token_capacity_bytes,
        limits.max_prepared_guard_token_bytes,
    )?;
    for (decision_ordinal, &decision) in decisions.iter().enumerate() {
        if decision.polarity() != ParametricSectorFormulaResidualPolarity::NonZero {
            continue;
        }
        let source_polynomial = path.structural_locus(decision_ordinal).ok_or(
            ParametricSectorFormulaAffineTerminalError::StructuralLocusOutOfRange {
                decision_ordinal,
            },
        )?;
        let call_limits = remaining_guard_composition_limits(limits, stats.guard_preflight)?;
        let token = context
            .prepare_guard_on_residual_affine_compact_composition_plan(
                source_polynomial,
                &compact_plan,
                call_limits.effective,
            )
            .map_err(|error| {
                map_guard_composition_error(error, call_limits, limits, stats.guard_preflight)
            })?;
        merge_guard_composition_stats(&mut stats.guard_preflight, token.stats(), limits)?;
        prepared.push((decision_ordinal, decision, token));
    }
    if prepared.len() != guard_count {
        return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
    }

    let mut guards = Vec::new();
    try_reserve_exact(&mut guards, guard_count, "affine-terminal guard entries")?;
    stats.retained_guard_entry_capacity_bytes = checked_mul(
        "affine-terminal retained guard entry bytes",
        guards.capacity(),
        size_of::<ParametricSectorFormulaAffineGuardEntry>(),
    )?;
    check_limit(
        "affine-terminal retained guard entry bytes",
        stats.retained_guard_entry_capacity_bytes,
        limits.max_retained_guard_entry_bytes,
    )?;
    let mut first_guard_contradiction = None;
    for (decision_ordinal, decision, token) in prepared {
        let guard_entry_ordinal = guards.len();
        let prospective = token.stats();
        let (mapped_polynomial, composition_stats) = token.execute()?.into_parts();
        if !execution_guard_fits_preflight(composition_stats, prospective) {
            return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
        }
        merge_guard_composition_stats(&mut stats.guard_execution, composition_stats, limits)?;
        let class = if mapped_polynomial.is_zero() {
            stats.guard_contradictions = checked_add(
                "affine-terminal guard contradictions",
                stats.guard_contradictions,
                1,
            )?;
            if first_guard_contradiction.is_none() {
                first_guard_contradiction = Some(
                    ParametricSectorFormulaAffineEmptyReason::MappedNonzeroGuardContradiction {
                        guard_entry_ordinal,
                        decision_ordinal,
                        decision,
                    },
                );
            }
            ParametricSectorFormulaAffineGuardClass::Contradiction(mapped_polynomial)
        } else if mapped_polynomial.is_nonzero_constant() {
            stats.discharged_nonzero_integer_constants = checked_add(
                "affine-terminal discharged integer guards",
                stats.discharged_nonzero_integer_constants,
                1,
            )?;
            ParametricSectorFormulaAffineGuardClass::DischargedNonzeroIntegerConstant(
                mapped_polynomial,
            )
        } else {
            check_limit(
                "affine-terminal per-condition guard origins",
                1,
                limits.guard_composition.max_guard_origins,
            )?;
            let origin = GuardOrigin::GeneratedAffineSealedCondition;
            stats.total_guard_origin_retained_bytes = bounded_add(
                "affine-terminal total guard origin retained bytes",
                stats.total_guard_origin_retained_bytes,
                guard_origin_bytes,
                limits.max_total_guard_origin_retained_bytes,
            )?;
            let depends_on_indices = context.polynomial_depends_on_indices_with_limits(
                &mapped_polynomial,
                limits.guard_composition.exact_algebra,
            )?;
            let condition = context.nonzero_condition_with_origins_and_origin_limit(
                mapped_polynomial,
                [origin],
                limits.guard_composition.exact_algebra,
                limits.guard_composition.max_guard_origins,
            )?;
            if depends_on_indices {
                stats.free_index_dependent_conditions = checked_add(
                    "affine-terminal free-index guard conditions",
                    stats.free_index_dependent_conditions,
                    1,
                )?;
                ParametricSectorFormulaAffineGuardClass::FreeIndexDependent(condition)
            } else {
                stats.base_assumptions = checked_add(
                    "affine-terminal base guard assumptions",
                    stats.base_assumptions,
                    1,
                )?;
                ParametricSectorFormulaAffineGuardClass::BaseAssumption(condition)
            }
        };
        guards.push(ParametricSectorFormulaAffineGuardEntry {
            decision_ordinal,
            decision,
            class,
            composition_stats,
        });
    }
    if !execution_guard_aggregate_fits_preflight(stats.guard_execution, stats.guard_preflight)
        || stats.total_guard_origin_retained_bytes > stats.guard_origin_retained_byte_envelope
    {
        return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
    }
    stats.guard_entries = guards.len();
    let outcome = first_guard_contradiction.map_or(
        ParametricSectorFormulaAffineTerminalOutcome::Actionable,
        ParametricSectorFormulaAffineTerminalOutcome::ProvedEmpty,
    );
    Ok(ParametricSectorFormulaAffineTerminalCertificate {
        schema: PARAMETRIC_SECTOR_FORMULA_AFFINE_TERMINAL_V1_SCHEMA,
        path,
        recognitions,
        unsupported_reasons,
        geometry: Some(geometry),
        compact_plan: Some(compact_plan),
        guards,
        outcome,
        limits,
        stats,
    })
}

fn apply_coordinate_decision(
    sector: &SectorMask,
    fixed: &mut [Option<FixedCoordinate>],
    excluded: &mut Vec<ExcludedCoordinate>,
    stats: &mut ParametricSectorFormulaAffineTerminalStats,
    limits: ParametricSectorFormulaAffineTerminalLimits,
    decision_ordinal: usize,
    decision: ParametricSectorFormulaResidualDecision,
    coordinate: Option<(usize, i64)>,
) -> Result<
    Option<ParametricSectorFormulaAffineEmptyReason>,
    ParametricSectorFormulaAffineTerminalError,
> {
    let Some((index, value)) = coordinate else {
        return Ok(None);
    };
    let active = sector
        .active_bits()
        .get(index)
        .copied()
        .ok_or(ParametricSectorFormulaAffineTerminalError::SourceShapeMismatch)?;
    let slot = fixed
        .get_mut(index)
        .ok_or(ParametricSectorFormulaAffineTerminalError::SourceShapeMismatch)?;
    match decision.polarity() {
        ParametricSectorFormulaResidualPolarity::EqualZero => {
            if (active && value < 1) || (!active && value > 0) {
                return Ok(Some(
                    ParametricSectorFormulaAffineEmptyReason::OrthantViolation {
                        decision_ordinal,
                        decision,
                        index,
                        value,
                        side: if active {
                            SectorOrthantSide::AtLeastOne
                        } else {
                            SectorOrthantSide::AtMostZero
                        },
                    },
                ));
            }
            if let Some(existing) = *slot {
                charge_coordinate_conflict_comparison(stats, limits)?;
                if existing.value != value {
                    return Ok(Some(
                        ParametricSectorFormulaAffineEmptyReason::ConflictingFixedValues {
                            first_decision_ordinal: existing.decision_ordinal,
                            first_decision: existing.decision,
                            second_decision_ordinal: decision_ordinal,
                            second_decision: decision,
                            index,
                            first_value: existing.value,
                            second_value: value,
                        },
                    ));
                }
            }
            for excluded_coordinate in excluded.iter() {
                charge_coordinate_conflict_comparison(stats, limits)?;
                if excluded_coordinate.index == index && excluded_coordinate.value == value {
                    return Ok(Some(
                        ParametricSectorFormulaAffineEmptyReason::EqualityNonzeroCoordinateConflict {
                            equality_decision_ordinal: decision_ordinal,
                            equality_decision: decision,
                            nonzero_decision_ordinal: excluded_coordinate.decision_ordinal,
                            nonzero_decision: excluded_coordinate.decision,
                            index,
                            value,
                        },
                    ));
                }
            }
            if slot.is_none() {
                *slot = Some(FixedCoordinate {
                    value,
                    decision_ordinal,
                    decision,
                });
            }
        }
        ParametricSectorFormulaResidualPolarity::NonZero => {
            if let Some(existing) = *slot {
                charge_coordinate_conflict_comparison(stats, limits)?;
                if existing.value == value {
                    return Ok(Some(
                        ParametricSectorFormulaAffineEmptyReason::EqualityNonzeroCoordinateConflict {
                            equality_decision_ordinal: existing.decision_ordinal,
                            equality_decision: existing.decision,
                            nonzero_decision_ordinal: decision_ordinal,
                            nonzero_decision: decision,
                            index,
                            value,
                        },
                    ));
                }
            }
            if excluded.len() == excluded.capacity() {
                return Err(ParametricSectorFormulaAffineTerminalError::ReplayMismatch);
            }
            excluded.push(ExcludedCoordinate {
                index,
                value,
                decision_ordinal,
                decision,
            });
        }
    }
    Ok(None)
}

fn charge_coordinate_conflict_comparison(
    stats: &mut ParametricSectorFormulaAffineTerminalStats,
    limits: ParametricSectorFormulaAffineTerminalLimits,
) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
    stats.coordinate_conflict_comparisons = bounded_add(
        "affine-terminal coordinate-conflict comparisons",
        stats.coordinate_conflict_comparisons,
        1,
        limits.max_coordinate_conflict_comparisons,
    )?;
    Ok(())
}

fn build_cylinder_geometry(
    fixed: &[Option<FixedCoordinate>],
    limits: ParametricSectorFormulaAffineTerminalLimits,
    stats: &mut ParametricSectorFormulaAffineTerminalStats,
) -> Result<ParametricSectorFormulaAffineCylinderGeometry, ParametricSectorFormulaAffineTerminalError>
{
    let fixed_count = fixed.iter().filter(|entry| entry.is_some()).count();
    let free_count = fixed.len().checked_sub(fixed_count).ok_or(
        ParametricSectorFormulaAffineTerminalError::ResourceCountOverflow {
            resource: "affine-terminal free positions",
        },
    )?;
    check_limit(
        "affine-terminal fixed coordinates",
        fixed_count,
        limits.max_fixed_coordinates,
    )?;
    check_limit(
        "affine-terminal free positions",
        free_count,
        limits.max_free_positions,
    )?;
    let matrix_entries = checked_mul(
        "affine-terminal compact matrix entries",
        fixed.len(),
        free_count,
    )?;
    check_limit(
        "affine-terminal compact matrix entries",
        matrix_entries,
        limits.max_compact_matrix_entries,
    )?;

    // Bound the logical retained cylinder before the first allocation.  The
    // post-reserve capacity census below remains authoritative for allocator
    // over-allocation.
    let cylinder_geometry_byte_envelope = checked_sum(
        "affine-terminal cylinder geometry capacity bytes",
        [
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                fixed.len(),
                size_of::<Integer>(),
            )?,
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                free_count,
                size_of::<usize>(),
            )?,
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                matrix_entries,
                size_of::<Integer>(),
            )?,
        ],
    )?;
    check_limit(
        "affine-terminal cylinder geometry capacity bytes",
        cylinder_geometry_byte_envelope,
        limits.max_cylinder_geometry_capacity_bytes,
    )?;
    stats.cylinder_geometry_byte_envelope = cylinder_geometry_byte_envelope;

    let mut constants = Vec::new();
    try_reserve_exact(
        &mut constants,
        fixed.len(),
        "affine-terminal cylinder constants",
    )?;
    let mut free_positions = Vec::new();
    try_reserve_exact(
        &mut free_positions,
        free_count,
        "affine-terminal cylinder free positions",
    )?;
    for (index, entry) in fixed.iter().enumerate() {
        constants.push(Integer::from(entry.map_or(0, |fixed| fixed.value)));
        if entry.is_none() {
            free_positions.push(index);
        }
    }
    let mut compact_linear_coefficients = Vec::new();
    try_reserve_exact(
        &mut compact_linear_coefficients,
        matrix_entries,
        "affine-terminal compact cylinder matrix",
    )?;
    for row in 0..fixed.len() {
        for &free_position in &free_positions {
            compact_linear_coefficients.push(Integer::from(i64::from(row == free_position)));
        }
    }
    let cylinder_geometry_capacity_bytes = checked_sum(
        "affine-terminal cylinder geometry capacity bytes",
        [
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                constants.capacity(),
                size_of::<Integer>(),
            )?,
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                free_positions.capacity(),
                size_of::<usize>(),
            )?,
            checked_mul(
                "affine-terminal cylinder geometry capacity bytes",
                compact_linear_coefficients.capacity(),
                size_of::<Integer>(),
            )?,
        ],
    )?;
    check_limit(
        "affine-terminal cylinder geometry capacity bytes",
        cylinder_geometry_capacity_bytes,
        limits.max_cylinder_geometry_capacity_bytes,
    )?;
    stats.fixed_coordinates = fixed_count;
    stats.free_positions = free_count;
    stats.compact_matrix_entries = matrix_entries;
    stats.cylinder_geometry_capacity_bytes = cylinder_geometry_capacity_bytes;
    Ok(ParametricSectorFormulaAffineCylinderGeometry {
        ambient_arity: fixed.len(),
        constants,
        free_positions,
        compact_linear_coefficients,
    })
}

fn remaining_guard_composition_limits(
    limits: ParametricSectorFormulaAffineTerminalLimits,
    consumed: ParametricSectorFormulaAffineGuardCompositionStats,
) -> Result<GuardCompositionCallLimits, ParametricSectorFormulaAffineTerminalError> {
    let mut effective = limits.guard_composition;
    let mut clamps = GuardCompositionClampProvenance::default();
    macro_rules! clamp_remaining {
        ($field:ident, $clamp:ident, $used:ident, $total:ident, $name:literal) => {{
            let aggregate_remaining = remaining($name, limits.$total, consumed.$used)?;
            if aggregate_remaining < effective.$field {
                effective.$field = aggregate_remaining;
                clamps.$clamp = true;
            }
        }};
    }
    clamp_remaining!(
        max_source_terms,
        source_terms,
        source_terms,
        max_total_guard_source_terms,
        "affine-terminal total guard source terms"
    );
    clamp_remaining!(
        max_source_exponent_entries,
        source_exponent_entries,
        source_exponent_entries,
        max_total_guard_source_exponent_entries,
        "affine-terminal total guard source exponent entries"
    );
    clamp_remaining!(
        max_expanded_contributions,
        expanded_contribution_bound,
        expanded_contribution_bound,
        max_total_guard_expanded_contribution_bound,
        "affine-terminal total guard expanded contribution bound"
    );

    // Two aggregate budgets conservatively constrain the same child bound.
    // Record both strict clamps against the original per-call limit, then
    // select their minimum.  This lets error remapping identify the exact
    // outer controller (with deterministic bound-before-actual tie-breaking).
    let local_output_terms = effective.max_output_terms;
    let output_term_bound_remaining = remaining(
        "affine-terminal total guard output term bound",
        limits.max_total_guard_output_term_bound,
        consumed.output_term_bound,
    )?;
    let output_terms_remaining = remaining(
        "affine-terminal total guard output terms",
        limits.max_total_guard_output_terms,
        consumed.output_term_bound,
    )?;
    clamps.output_term_bound = output_term_bound_remaining < local_output_terms;
    clamps.output_terms = output_terms_remaining < local_output_terms;
    effective.max_output_terms = local_output_terms
        .min(output_term_bound_remaining)
        .min(output_terms_remaining);

    let local_output_exponent_entries = effective.max_output_exponent_entries;
    let output_exponent_entry_bound_remaining = remaining(
        "affine-terminal total guard output exponent-entry bound",
        limits.max_total_guard_output_exponent_entry_bound,
        consumed.output_exponent_entry_bound,
    )?;
    let output_exponent_entries_remaining = remaining(
        "affine-terminal total guard output exponent entries",
        limits.max_total_guard_output_exponent_entries,
        consumed.output_exponent_entry_bound,
    )?;
    clamps.output_exponent_entry_bound =
        output_exponent_entry_bound_remaining < local_output_exponent_entries;
    clamps.output_exponent_entries =
        output_exponent_entries_remaining < local_output_exponent_entries;
    effective.max_output_exponent_entries = local_output_exponent_entries
        .min(output_exponent_entry_bound_remaining)
        .min(output_exponent_entries_remaining);

    clamp_remaining!(
        max_power_calls,
        power_calls,
        power_calls,
        max_total_guard_power_calls,
        "affine-terminal total guard power calls"
    );
    clamp_remaining!(
        max_native_power_heap_pairs,
        native_power_heap_pairs,
        native_power_heap_pairs,
        max_total_guard_native_power_heap_pairs,
        "affine-terminal total guard native power heap pairs"
    );
    clamp_remaining!(
        max_multiplication_term_pairs,
        multiplication_term_pairs,
        multiplication_term_pairs,
        max_total_guard_multiplication_term_pairs,
        "affine-terminal total guard multiplication term pairs"
    );
    clamp_remaining!(
        max_addition_term_visits,
        addition_term_visits,
        addition_term_visits,
        max_total_guard_addition_term_visits,
        "affine-terminal total guard addition term visits"
    );
    clamp_remaining!(
        max_native_integer_bit_work,
        native_integer_bit_work,
        native_integer_bit_work_bound,
        max_total_guard_native_integer_bit_work_bound,
        "affine-terminal total guard native integer-bit work bound"
    );
    clamp_remaining!(
        max_integer_bit_work,
        integer_bit_work,
        integer_bit_work_bound,
        max_total_guard_integer_bit_work_bound,
        "affine-terminal total guard integer-bit work bound"
    );
    Ok(GuardCompositionCallLimits { effective, clamps })
}

fn map_guard_composition_error(
    error: ResidualUnitAffineCompositionError,
    call: GuardCompositionCallLimits,
    limits: ParametricSectorFormulaAffineTerminalLimits,
    consumed: ParametricSectorFormulaAffineGuardCompositionStats,
) -> ParametricSectorFormulaAffineTerminalError {
    let ResidualUnitAffineCompositionError::ResourceLimit {
        resource,
        requested,
        limit: child_limit,
    } = error
    else {
        return ParametricSectorFormulaAffineTerminalError::Composition(error);
    };

    let remap = |outer_resource: &'static str,
                 spent_before_call: usize,
                 effective_call_limit: usize,
                 outer_limit: usize| {
        let Some(spent_inside_call) = effective_call_limit.checked_sub(child_limit) else {
            return ParametricSectorFormulaAffineTerminalError::Composition(
                ResidualUnitAffineCompositionError::ResourceLimit {
                    resource,
                    requested,
                    limit: child_limit,
                },
            );
        };
        match checked_sum(
            outer_resource,
            [spent_before_call, spent_inside_call, requested],
        ) {
            Ok(requested) => ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: outer_resource,
                requested,
                limit: outer_limit,
            },
            Err(error) => error,
        }
    };

    macro_rules! direct_remap {
        ($clamp:ident, $field:ident, $used:ident, $outer:ident, $outer_name:literal, [$($child_name:literal),+ $(,)?]) => {
            if call.clamps.$clamp && matches!(resource, $($child_name)|+) {
                return remap(
                    $outer_name,
                    consumed.$used,
                    call.effective.$field,
                    limits.$outer,
                );
            }
        };
    }

    direct_remap!(
        source_terms,
        max_source_terms,
        source_terms,
        max_total_guard_source_terms,
        "affine-terminal total guard source terms",
        ["polynomial source terms"]
    );
    direct_remap!(
        source_exponent_entries,
        max_source_exponent_entries,
        source_exponent_entries,
        max_total_guard_source_exponent_entries,
        "affine-terminal total guard source exponent entries",
        ["polynomial source exponent entries"]
    );

    let output_term_bound_remaining = limits
        .max_total_guard_output_term_bound
        .checked_sub(consumed.output_term_bound);
    let output_terms_remaining = limits
        .max_total_guard_output_terms
        .checked_sub(consumed.output_term_bound);
    let output_controller = if call.clamps.output_term_bound
        && output_term_bound_remaining == Some(call.effective.max_output_terms)
    {
        Some((
            "affine-terminal total guard output term bound",
            limits.max_total_guard_output_term_bound,
        ))
    } else if call.clamps.output_terms
        && output_terms_remaining == Some(call.effective.max_output_terms)
    {
        Some((
            "affine-terminal total guard output terms",
            limits.max_total_guard_output_terms,
        ))
    } else {
        None
    };

    // `heap_pow` reports one shared cap: min(expanded, output, exact
    // algebra). Remap only when an aggregate-clamped field controls that
    // minimum. A tie with an unclamped sibling remains a child Composition;
    // a tie between both aggregate-controlled fields resolves to expanded.
    if resource == "affine power terms"
        && child_limit < call.effective.exact_algebra.max_polynomial_terms
    {
        if call.clamps.expanded_contribution_bound
            && (call.effective.max_expanded_contributions < call.effective.max_output_terms
                || (call.effective.max_expanded_contributions == call.effective.max_output_terms
                    && output_controller.is_some()))
        {
            return remap(
                "affine-terminal total guard expanded contribution bound",
                consumed.expanded_contribution_bound,
                call.effective.max_expanded_contributions,
                limits.max_total_guard_expanded_contribution_bound,
            );
        }
        if call.effective.max_output_terms < call.effective.max_expanded_contributions {
            if let Some((outer_resource, outer_limit)) = output_controller {
                return remap(
                    outer_resource,
                    consumed.output_term_bound,
                    call.effective.max_output_terms,
                    outer_limit,
                );
            }
        }
    }
    direct_remap!(
        expanded_contribution_bound,
        max_expanded_contributions,
        expanded_contribution_bound,
        max_total_guard_expanded_contribution_bound,
        "affine-terminal total guard expanded contribution bound",
        ["expanded polynomial contributions"]
    );
    if resource == "prospective output terms" {
        if let Some((outer_resource, outer_limit)) = output_controller {
            return remap(
                outer_resource,
                consumed.output_term_bound,
                call.effective.max_output_terms,
                outer_limit,
            );
        }
    }

    let output_exponent_entry_bound_remaining = limits
        .max_total_guard_output_exponent_entry_bound
        .checked_sub(consumed.output_exponent_entry_bound);
    let output_exponent_entries_remaining = limits
        .max_total_guard_output_exponent_entries
        .checked_sub(consumed.output_exponent_entry_bound);
    if resource == "prospective output exponent entries" {
        if call.clamps.output_exponent_entry_bound
            && output_exponent_entry_bound_remaining
                == Some(call.effective.max_output_exponent_entries)
        {
            return remap(
                "affine-terminal total guard output exponent-entry bound",
                consumed.output_exponent_entry_bound,
                call.effective.max_output_exponent_entries,
                limits.max_total_guard_output_exponent_entry_bound,
            );
        }
        if call.clamps.output_exponent_entries
            && output_exponent_entries_remaining == Some(call.effective.max_output_exponent_entries)
        {
            return remap(
                "affine-terminal total guard output exponent entries",
                consumed.output_exponent_entry_bound,
                call.effective.max_output_exponent_entries,
                limits.max_total_guard_output_exponent_entries,
            );
        }
    }
    direct_remap!(
        power_calls,
        max_power_calls,
        power_calls,
        max_total_guard_power_calls,
        "affine-terminal total guard power calls",
        ["native power calls"]
    );
    direct_remap!(
        native_power_heap_pairs,
        max_native_power_heap_pairs,
        native_power_heap_pairs,
        max_total_guard_native_power_heap_pairs,
        "affine-terminal total guard native power heap pairs",
        ["native power heap pairs"]
    );
    direct_remap!(
        multiplication_term_pairs,
        max_multiplication_term_pairs,
        multiplication_term_pairs,
        max_total_guard_multiplication_term_pairs,
        "affine-terminal total guard multiplication term pairs",
        ["native multiplication term pairs"]
    );
    direct_remap!(
        addition_term_visits,
        max_addition_term_visits,
        addition_term_visits,
        max_total_guard_addition_term_visits,
        "affine-terminal total guard addition term visits",
        [
            "native addition term visits",
            "Symbolica backend structural term visits"
        ]
    );
    direct_remap!(
        native_integer_bit_work,
        max_native_integer_bit_work,
        native_integer_bit_work_bound,
        max_total_guard_native_integer_bit_work_bound,
        "affine-terminal total guard native integer-bit work bound",
        ["native integer bit work"]
    );
    direct_remap!(
        integer_bit_work,
        max_integer_bit_work,
        integer_bit_work_bound,
        max_total_guard_integer_bit_work_bound,
        "affine-terminal total guard integer-bit work bound",
        [
            "integer bit work",
            "coefficient total integer-bit work bound"
        ]
    );

    ParametricSectorFormulaAffineTerminalError::Composition(
        ResidualUnitAffineCompositionError::ResourceLimit {
            resource,
            requested,
            limit: child_limit,
        },
    )
}

fn merge_guard_composition_stats(
    aggregate: &mut ParametricSectorFormulaAffineGuardCompositionStats,
    item: ResidualUnitAffinePolynomialCompositionStats,
    limits: ParametricSectorFormulaAffineTerminalLimits,
) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
    macro_rules! add {
        ($field:ident, $value:expr, $limit:ident, $name:literal) => {
            aggregate.$field = bounded_add($name, aggregate.$field, $value, limits.$limit)?;
        };
    }
    add!(
        source_terms,
        item.source_terms(),
        max_total_guard_source_terms,
        "affine-terminal total guard source terms"
    );
    add!(
        source_exponent_entries,
        item.source_exponent_entries(),
        max_total_guard_source_exponent_entries,
        "affine-terminal total guard source exponent entries"
    );
    add!(
        expanded_contribution_bound,
        item.expanded_contribution_bound(),
        max_total_guard_expanded_contribution_bound,
        "affine-terminal total guard expanded contribution bound"
    );
    add!(
        output_term_bound,
        item.expanded_contribution_bound(),
        max_total_guard_output_term_bound,
        "affine-terminal total guard output term bound"
    );
    add!(
        output_terms,
        item.output_terms(),
        max_total_guard_output_terms,
        "affine-terminal total guard output terms"
    );
    add!(
        output_exponent_entry_bound,
        item.output_exponent_entry_bound(),
        max_total_guard_output_exponent_entry_bound,
        "affine-terminal total guard output exponent-entry bound"
    );
    add!(
        output_exponent_entries,
        item.output_exponent_entries(),
        max_total_guard_output_exponent_entries,
        "affine-terminal total guard output exponent entries"
    );
    add!(
        power_calls,
        item.power_calls(),
        max_total_guard_power_calls,
        "affine-terminal total guard power calls"
    );
    add!(
        native_power_heap_pairs,
        item.native_power_heap_pair_bound(),
        max_total_guard_native_power_heap_pairs,
        "affine-terminal total guard native power heap pairs"
    );
    add!(
        multiplication_term_pairs,
        item.multiplication_term_pair_bound(),
        max_total_guard_multiplication_term_pairs,
        "affine-terminal total guard multiplication term pairs"
    );
    add!(
        addition_term_visits,
        item.addition_term_visit_bound(),
        max_total_guard_addition_term_visits,
        "affine-terminal total guard addition term visits"
    );
    aggregate.largest_kronecker_exponent_bits = aggregate
        .largest_kronecker_exponent_bits
        .max(item.largest_kronecker_exponent_bits());
    aggregate.largest_integer_coefficient_bit_bound = aggregate
        .largest_integer_coefficient_bit_bound
        .max(item.largest_integer_coefficient_bit_bound());
    add!(
        native_integer_bit_work_bound,
        item.native_integer_bit_work_bound(),
        max_total_guard_native_integer_bit_work_bound,
        "affine-terminal total guard native integer-bit work bound"
    );
    add!(
        integer_bit_work_bound,
        item.integer_bit_work_bound(),
        max_total_guard_integer_bit_work_bound,
        "affine-terminal total guard integer-bit work bound"
    );
    Ok(())
}

fn execution_guard_fits_preflight(
    actual: ResidualUnitAffinePolynomialCompositionStats,
    prospective: ResidualUnitAffinePolynomialCompositionStats,
) -> bool {
    actual.source_terms() == prospective.source_terms()
        && actual.source_exponent_entries() == prospective.source_exponent_entries()
        && actual.expanded_contribution_bound() == prospective.expanded_contribution_bound()
        && actual.output_exponent_entry_bound() == prospective.output_exponent_entry_bound()
        && actual.power_calls() == prospective.power_calls()
        && actual.native_power_heap_pair_bound() == prospective.native_power_heap_pair_bound()
        && actual.multiplication_term_pair_bound() == prospective.multiplication_term_pair_bound()
        && actual.addition_term_visit_bound() == prospective.addition_term_visit_bound()
        && actual.largest_kronecker_exponent_bits() == prospective.largest_kronecker_exponent_bits()
        && actual.largest_integer_coefficient_bit_bound()
            == prospective.largest_integer_coefficient_bit_bound()
        && actual.native_integer_bit_work_bound() == prospective.native_integer_bit_work_bound()
        && actual.output_terms() <= prospective.expanded_contribution_bound()
        && actual.output_exponent_entries() <= prospective.output_exponent_entry_bound()
        && actual.integer_bit_work_bound() <= prospective.integer_bit_work_bound()
}

fn execution_guard_aggregate_fits_preflight(
    actual: ParametricSectorFormulaAffineGuardCompositionStats,
    prospective: ParametricSectorFormulaAffineGuardCompositionStats,
) -> bool {
    actual.source_terms == prospective.source_terms
        && actual.source_exponent_entries == prospective.source_exponent_entries
        && actual.expanded_contribution_bound == prospective.expanded_contribution_bound
        && actual.output_term_bound == prospective.output_term_bound
        && actual.output_exponent_entry_bound == prospective.output_exponent_entry_bound
        && actual.power_calls == prospective.power_calls
        && actual.native_power_heap_pairs == prospective.native_power_heap_pairs
        && actual.multiplication_term_pairs == prospective.multiplication_term_pairs
        && actual.addition_term_visits == prospective.addition_term_visits
        && actual.largest_kronecker_exponent_bits == prospective.largest_kronecker_exponent_bits
        && actual.largest_integer_coefficient_bit_bound
            == prospective.largest_integer_coefficient_bit_bound
        && actual.native_integer_bit_work_bound == prospective.native_integer_bit_work_bound
        && actual.output_terms <= prospective.output_term_bound
        && actual.output_exponent_entries <= prospective.output_exponent_entry_bound
        && actual.integer_bit_work_bound <= prospective.integer_bit_work_bound
}

fn remaining(
    resource: &'static str,
    limit: usize,
    consumed: usize,
) -> Result<usize, ParametricSectorFormulaAffineTerminalError> {
    limit
        .checked_sub(consumed)
        .ok_or(ParametricSectorFormulaAffineTerminalError::ResourceLimit {
            resource,
            requested: consumed,
            limit,
        })
}

fn push_unsupported_reason(
    reasons: &mut Vec<ParametricSectorFormulaAffineUnsupportedReason>,
    reason: ParametricSectorFormulaAffineUnsupportedReason,
    limits: ParametricSectorFormulaAffineTerminalLimits,
) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
    let requested = checked_add("affine-terminal unsupported reasons", reasons.len(), 1)?;
    check_limit(
        "affine-terminal unsupported reasons",
        requested,
        limits.max_unsupported_reasons,
    )?;
    if reasons.len() == reasons.capacity() {
        reasons.try_reserve_exact(1).map_err(|_| {
            ParametricSectorFormulaAffineTerminalError::AllocationFailure {
                resource: "affine-terminal unsupported reasons",
                requested,
            }
        })?;
    }
    reasons.push(reason);
    Ok(())
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
    if requested > limit {
        Err(ParametricSectorFormulaAffineTerminalError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorFormulaAffineTerminalError> {
    left.checked_add(right)
        .ok_or(ParametricSectorFormulaAffineTerminalError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricSectorFormulaAffineTerminalError> {
    left.checked_mul(right)
        .ok_or(ParametricSectorFormulaAffineTerminalError::ResourceCountOverflow { resource })
}

fn checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, ParametricSectorFormulaAffineTerminalError> {
    values
        .into_iter()
        .try_fold(0usize, |sum, value| checked_add(resource, sum, value))
}

fn bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, ParametricSectorFormulaAffineTerminalError> {
    let requested = checked_add(resource, left, right)?;
    check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    resource: &'static str,
) -> Result<(), ParametricSectorFormulaAffineTerminalError> {
    values.try_reserve_exact(additional).map_err(|_| {
        ParametricSectorFormulaAffineTerminalError::AllocationFailure {
            resource,
            requested: additional,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parametric_sector_formula_residual::{
        ParametricSectorFormulaResidualCursor, ParametricSectorFormulaResidualLimits,
        ParametricSectorFormulaResidualRequest,
    };
    use crate::parametric_sector_normalized_source::{
        ParametricSectorNormalizedCoverageSource, ParametricSectorNormalizedCoverageSourceCompiler,
        ParametricSectorNormalizedCoverageSourceLimits,
    };
    use crate::{
        AffineDenominator, CoefficientContext, GeneratedSectorDiscoveryCompiler,
        GeneratedSectorDiscoveryLimits, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
        GeneratedWhenBadLimits, ParametricElimination, ParametricEliminationLimits,
        ParametricEliminationOrdering, ParametricIbpGenerator, ParametricReductionRuleCandidate,
        ParametricRuleLimits,
    };

    fn massive_tadpole(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        IntegralFamily::new(
            name,
            vec!["k".into()],
            Vec::new(),
            coefficients.clone(),
            coefficients.parameter("d").unwrap(),
            vec![AffineDenominator::new(
                coefficients.parse("-m2").unwrap(),
                vec![coefficients.one()],
            )],
            Vec::new(),
            vec![coefficients.zero()],
        )
        .unwrap()
    }

    fn massive_sunset(name: &str) -> IntegralFamily {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let zero = coefficients.zero();
        let one = coefficients.one();
        let minus_m2 = coefficients.parse("-m2").unwrap();
        IntegralFamily::new(
            name,
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
        .unwrap()
    }

    fn one_loop_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorNormalizedCoverageSource>,
    ) {
        let family = massive_tadpole(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_new([true]).unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let compilations = discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect();
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                compilations,
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn unsupported_one_loop_source() -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorNormalizedCoverageSource>,
    ) {
        let family = massive_tadpole("affine-terminal-unsupported-one-loop");
        let generated = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .generate()
            .unwrap();
        let context = generated.context().clone();
        let rows = generated.ibp_li().cloned().collect::<Vec<_>>();
        let sector = SectorMask::try_new([false]).unwrap();
        let elimination = ParametricElimination::build(
            &context,
            &rows,
            ParametricEliminationOrdering::try_new(IntegralOrderingPolicy::RustRedUnshiftedV1, [0])
                .unwrap(),
            ParametricEliminationLimits::default(),
        )
        .unwrap();
        let candidate = ParametricReductionRuleCandidate::try_from_elimination_pivot(
            &context,
            &rows,
            &elimination,
            0,
            sector.clone(),
            ParametricRuleLimits::default(),
        )
        .unwrap();
        let compilation = GeneratedWhenBadCompiler::compile(
            &family,
            &context,
            &candidate,
            GeneratedWhenBadLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            compilation,
            GeneratedWhenBadCompilation::Unsupported(_)
        ));
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                sector,
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                vec![compilation.clone(), compilation],
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn sunset_source(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Arc<ParametricSectorNormalizedCoverageSource>,
    ) {
        let family = massive_sunset(name);
        let context = ParametricIbpGenerator::try_new(&family)
            .unwrap()
            .context()
            .clone();
        let mut discovery_limits = GeneratedSectorDiscoveryLimits::default();
        discovery_limits.adaptive.max_search_depth = 0;
        discovery_limits
            .coverage
            .max_materialized_product_zero_support_terms = 0;
        let discovery = GeneratedSectorDiscoveryCompiler::compile(
            &family,
            &context,
            SectorMask::try_from_bit_string("111").unwrap(),
            IntegralOrderingPolicy::RustRedUnshiftedV1,
            discovery_limits,
        )
        .unwrap();
        let compilations = discovery
            .coverage()
            .candidate_attempts()
            .iter()
            .map(|attempt| attempt.compilation().clone())
            .collect();
        let source = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                discovery.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                compilations,
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        (family, context, source)
    }

    fn first_path(
        family: &IntegralFamily,
        context: &ParametricCoefficientContext,
        source: Arc<ParametricSectorNormalizedCoverageSource>,
        request: ParametricSectorFormulaResidualRequest,
    ) -> Arc<ParametricSectorFormulaResidualPathCertificate> {
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            family,
            context,
            source,
            request,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        Arc::new(cursor.next_path().unwrap().unwrap())
    }

    fn generated_sunset_paths(
        name: &str,
    ) -> (
        IntegralFamily,
        ParametricCoefficientContext,
        Vec<Arc<ParametricSectorFormulaResidualPathCertificate>>,
    ) {
        let (family, context, source) = sunset_source(name);
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let mut paths = Vec::new();
        while let Some(path) = cursor.next_path().unwrap() {
            paths.push(Arc::new(path));
        }
        assert_eq!(paths.len(), 9);
        (family, context, paths)
    }

    #[test]
    fn empty_source_is_replayable_actionable_identity_with_tight_geometry_limits() {
        let (family, context, nonempty) = one_loop_source("affine-terminal-empty-source");
        let empty = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                nonempty.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        let path = first_path(
            &family,
            &context,
            empty,
            ParametricSectorFormulaResidualRequest::Uncovered,
        );
        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_ambient_arity = 1;
        exact.max_decisions = 0;
        exact.max_recognition_entries = 0;
        exact.max_unsupported_reasons = 0;
        exact.max_fixed_coordinates = 0;
        exact.max_free_positions = 1;
        exact.max_compact_matrix_entries = 1;
        exact.max_guard_entries = 0;
        let terminal = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&path),
            exact,
        )
        .unwrap();
        assert!(terminal.same_path_allocation(&path));
        assert_eq!(
            terminal.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::Actionable
        );
        let geometry = terminal.geometry().unwrap();
        assert_eq!(geometry.ambient_arity(), 1);
        assert_eq!(geometry.constants(), &[Integer::from(0)]);
        assert_eq!(geometry.free_positions(), &[0]);
        assert_eq!(geometry.compact_linear_coefficients(), &[Integer::from(1)]);
        assert!(terminal.guards().is_empty());
        assert_eq!(
            terminal.ordering_policy(),
            IntegralOrderingPolicy::RustRedUnshiftedV1
        );
        terminal.replay(&family, &context).unwrap();

        let mut below = exact;
        below.max_compact_matrix_entries = 0;
        assert!(matches!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(&family, &context, path, below,),
            Err(ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal compact matrix entries",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn durable_identity_is_allocation_independent_limit_sensitive_and_exactly_bounded() {
        fn compile(
            name: &str,
            max_ambient_arity: usize,
        ) -> ParametricSectorFormulaAffineTerminalCertificate {
            let (family, context, nonempty) = one_loop_source(name);
            let empty = Arc::new(
                ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                    &family,
                    &context,
                    nonempty.sector().clone(),
                    IntegralOrderingPolicy::RustRedUnshiftedV1,
                    Vec::new(),
                    ParametricSectorNormalizedCoverageSourceLimits::default(),
                )
                .unwrap(),
            );
            let path = first_path(
                &family,
                &context,
                empty,
                ParametricSectorFormulaResidualRequest::Uncovered,
            );
            let mut limits = ParametricSectorFormulaAffineTerminalLimits::default();
            limits.max_ambient_arity = max_ambient_arity;
            ParametricSectorFormulaAffineTerminalCompiler::compile(&family, &context, path, limits)
                .unwrap()
        }

        let left = compile("affine-terminal-durable-identity", 1);
        let right = compile("affine-terminal-durable-identity", 1);
        assert!(!left.same_path_allocation(right.path_arc()));
        let left_identity = left
            .encode_durable_identity(ExactIdentityLimits::default())
            .unwrap();
        let right_identity = right
            .encode_durable_identity(ExactIdentityLimits::default())
            .unwrap();
        assert_eq!(left_identity.as_str(), right_identity.as_str());

        let looser = compile("affine-terminal-durable-identity", 2);
        let looser_identity = looser
            .encode_durable_identity(ExactIdentityLimits::default())
            .unwrap();
        assert_ne!(left_identity.as_str(), looser_identity.as_str());

        let stats = left_identity.stats();
        let exact = ExactIdentityLimits {
            max_identity_bytes: stats.identity_bytes(),
            max_fields: stats.fields(),
            max_tag_bytes: stats.tag_bytes(),
            max_string_values: stats.string_values(),
            max_string_bytes: stats.string_bytes(),
            max_nesting_depth: stats.maximum_nesting_depth(),
            max_polynomials: stats.polynomials(),
            max_polynomial_variables: stats.polynomial_variables(),
            max_polynomial_terms: stats.polynomial_terms(),
            max_exponent_entries: stats.exponent_entries(),
            max_integers: stats.integers(),
            max_integer_bits: stats.integer_bits(),
        };
        assert_eq!(
            left.encode_durable_identity(exact).unwrap().as_str(),
            left_identity.as_str()
        );
        let mut one_below = exact;
        one_below.max_identity_bytes -= 1;
        assert!(matches!(
            left.encode_durable_identity(one_below),
            Err(ExactIdentityError::ResourceLimit {
                resource: "exact structural identity bytes",
                requested,
                limit,
            }) if requested == stats.identity_bytes() && limit + 1 == requested
        ));
    }

    #[test]
    fn durable_identity_encodes_nonempty_generated_attempt_chain_exactly() {
        fn compile(
            name: &str,
        ) -> (
            IntegralFamily,
            ParametricCoefficientContext,
            ParametricSectorFormulaAffineTerminalCertificate,
        ) {
            let (family, context, source) = one_loop_source(name);
            assert!(!source.attempts().is_empty());
            let path = first_path(
                &family,
                &context,
                source,
                ParametricSectorFormulaResidualRequest::AnyResidual,
            );
            let terminal = ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                path,
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap();
            (family, context, terminal)
        }

        let (left_family, left_context, left) =
            compile("affine-terminal-nonempty-durable-identity");
        let (right_family, right_context, right) =
            compile("affine-terminal-nonempty-durable-identity");
        left.replay(&left_family, &left_context).unwrap();
        right.replay(&right_family, &right_context).unwrap();
        assert!(!left.same_path_allocation(right.path_arc()));

        let left_identity = left
            .encode_durable_identity(ExactIdentityLimits::default())
            .unwrap();
        let right_identity = right
            .encode_durable_identity(ExactIdentityLimits::default())
            .unwrap();
        assert_eq!(left_identity.as_str(), right_identity.as_str());
        assert!(left_identity.as_str().contains("source_authentication"));
        assert!(left_identity.as_str().contains("admissibility"));

        let stats = left_identity.stats();
        let exact = ExactIdentityLimits {
            max_identity_bytes: stats.identity_bytes(),
            max_fields: stats.fields(),
            max_tag_bytes: stats.tag_bytes(),
            max_string_values: stats.string_values(),
            max_string_bytes: stats.string_bytes(),
            max_nesting_depth: stats.maximum_nesting_depth(),
            max_polynomials: stats.polynomials(),
            max_polynomial_variables: stats.polynomial_variables(),
            max_polynomial_terms: stats.polynomial_terms(),
            max_exponent_entries: stats.exponent_entries(),
            max_integers: stats.integers(),
            max_integer_bits: stats.integer_bits(),
        };
        assert_eq!(
            left.encode_durable_identity(exact).unwrap().as_str(),
            left_identity.as_str()
        );
        let mut one_below = exact;
        one_below.max_fields -= 1;
        assert!(matches!(
            left.encode_durable_identity(one_below),
            Err(ExactIdentityError::ResourceLimit {
                resource: "exact structural identity fields",
                requested,
                limit,
            }) if requested <= stats.fields() && requested > limit && limit + 1 == stats.fields()
        ));
    }

    #[test]
    fn durable_identity_distinguishes_residual_requests_with_same_terminal_geometry() {
        let (family, context, nonempty) = one_loop_source("affine-terminal-request-identity");
        let empty = Arc::new(
            ParametricSectorNormalizedCoverageSourceCompiler::compile_authenticated(
                &family,
                &context,
                nonempty.sector().clone(),
                IntegralOrderingPolicy::RustRedUnshiftedV1,
                Vec::new(),
                ParametricSectorNormalizedCoverageSourceLimits::default(),
            )
            .unwrap(),
        );
        let any_path = first_path(
            &family,
            &context,
            Arc::clone(&empty),
            ParametricSectorFormulaResidualRequest::AnyResidual,
        );
        let uncovered_path = first_path(
            &family,
            &context,
            empty,
            ParametricSectorFormulaResidualRequest::Uncovered,
        );
        let any = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            any_path,
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let uncovered = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            uncovered_path,
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        assert_eq!(any.outcome(), uncovered.outcome());
        assert_eq!(any.geometry(), uncovered.geometry());
        assert_eq!(any.recognitions(), uncovered.recognitions());
        assert_ne!(
            any.encode_durable_identity(ExactIdentityLimits::default())
                .unwrap()
                .as_str(),
            uncovered
                .encode_durable_identity(ExactIdentityLimits::default())
                .unwrap()
                .as_str()
        );
    }

    #[test]
    fn generated_active_one_loop_path_fixes_coordinate_and_preserves_guard_order() {
        let (family, context, source) = one_loop_source("affine-terminal-active-one-loop");
        let mut cursor = ParametricSectorFormulaResidualCursor::try_new(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::AnyResidual,
            ParametricSectorFormulaResidualLimits::default(),
        )
        .unwrap();
        let mut selected = None;
        while let Some(path) = cursor.next_path().unwrap() {
            let path = Arc::new(path);
            let terminal = ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&path),
                ParametricSectorFormulaAffineTerminalLimits::default(),
            )
            .unwrap();
            if terminal.outcome() == ParametricSectorFormulaAffineTerminalOutcome::Actionable
                && terminal
                    .geometry()
                    .is_some_and(|geometry| geometry.free_positions().is_empty())
            {
                selected = Some((path, terminal));
                break;
            }
        }
        let (path, terminal) =
            selected.expect("generated one-loop source has a fixed actionable path");
        assert!(terminal.same_path_allocation(&path));
        assert_eq!(
            terminal.geometry().unwrap().constants(),
            &[Integer::from(1)]
        );
        assert_eq!(terminal.recognitions().len(), path.decisions().len());
        assert!(
            terminal
                .recognitions()
                .iter()
                .enumerate()
                .all(
                    |(ordinal, recognition)| recognition.decision_ordinal() == ordinal
                        && recognition.decision() == path.decisions()[ordinal]
                )
        );
        let expected_nonzero_ordinals = path
            .decisions()
            .iter()
            .enumerate()
            .filter_map(|(ordinal, decision)| {
                (decision.polarity() == ParametricSectorFormulaResidualPolarity::NonZero)
                    .then_some(ordinal)
            })
            .collect::<Vec<_>>();
        assert_eq!(
            terminal
                .guards()
                .iter()
                .map(ParametricSectorFormulaAffineGuardEntry::decision_ordinal)
                .collect::<Vec<_>>(),
            expected_nonzero_ordinals
        );
        terminal.replay(&family, &context).unwrap();
    }

    #[test]
    fn unsupported_source_terminal_stays_typed_and_source_backed() {
        let (family, context, source) = unsupported_one_loop_source();
        let path = first_path(
            &family,
            &context,
            source,
            ParametricSectorFormulaResidualRequest::Unsupported,
        );
        let terminal = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&path),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        assert!(terminal.same_path_allocation(&path));
        assert_eq!(
            terminal.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::Unsupported
        );
        assert_eq!(
            terminal.unsupported_reasons(),
            &[ParametricSectorFormulaAffineUnsupportedReason::SourceTerminalUnsupported]
        );
        assert!(terminal.geometry().is_none());
        assert!(terminal.guards().is_empty());
        assert_eq!(
            path.unsupported_candidate_ordinals().collect::<Vec<_>>(),
            [0, 1]
        );
        terminal.replay(&family, &context).unwrap();
    }

    #[test]
    fn generated_sunset_paths_cover_orthant_conflict_and_fully_fixed_actionable_cases() {
        let (family, context, paths) = generated_sunset_paths("affine-terminal-generated-sunset");

        let base_assumptions = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[0]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        assert_eq!(
            base_assumptions.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::Actionable
        );
        let base_geometry = base_assumptions.geometry().unwrap();
        assert_eq!(
            base_geometry.constants(),
            &[
                Integer::from(i64::MAX),
                Integer::from(0),
                Integer::from(i64::MAX)
            ]
        );
        assert_eq!(base_geometry.free_positions(), &[1]);
        assert_eq!(
            base_assumptions
                .guards()
                .iter()
                .map(ParametricSectorFormulaAffineGuardEntry::decision_ordinal)
                .collect::<Vec<_>>(),
            [0, 2, 3]
        );
        assert_eq!(
            base_assumptions
                .guards()
                .iter()
                .map(|guard| guard.decision().structural_locus_ordinal())
                .collect::<Vec<_>>(),
            [0, 3, 4]
        );
        for guard in base_assumptions.guards() {
            assert_eq!(
                guard.decision(),
                paths[0].decisions()[guard.decision_ordinal()]
            );
            let ParametricSectorFormulaAffineGuardClass::BaseAssumption(condition) = guard.class()
            else {
                panic!("generated path 0 guards must be base assumptions");
            };
            assert_eq!(condition.origins().len(), 1);
            assert!(
                condition
                    .origins()
                    .contains(&GuardOrigin::GeneratedAffineSealedCondition)
            );
            assert!(std::ptr::eq(
                guard.mapped_polynomial(),
                condition.polynomial()
            ));
        }
        assert_eq!(base_assumptions.stats().base_assumptions(), 3);
        assert_eq!(
            base_assumptions.stats().total_guard_origin_retained_bytes(),
            base_assumptions
                .stats()
                .guard_origin_retained_byte_envelope()
        );
        base_assumptions.replay(&family, &context).unwrap();

        let mut non_actionable_origin_limits =
            ParametricSectorFormulaAffineTerminalLimits::default();
        non_actionable_origin_limits
            .guard_composition
            .max_guard_origins = 0;
        non_actionable_origin_limits
            .guard_composition
            .max_guard_origin_retained_bytes = 0;
        non_actionable_origin_limits.max_total_guard_origin_retained_bytes = 0;
        assert!(paths[1].decisions().iter().any(|decision| {
            decision.polarity() == ParametricSectorFormulaResidualPolarity::NonZero
        }));
        let orthant = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[1]),
            non_actionable_origin_limits,
        )
        .unwrap();
        assert!(orthant.same_path_allocation(&paths[1]));
        assert!(matches!(
            orthant.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::ProvedEmpty(
                ParametricSectorFormulaAffineEmptyReason::OrthantViolation {
                    decision_ordinal: 3,
                    index: 0,
                    value: 0,
                    side: SectorOrthantSide::AtLeastOne,
                    ..
                }
            )
        ));
        assert!(orthant.geometry().is_none());
        assert_eq!(orthant.stats().guard_origin_retained_byte_envelope(), 0);
        orthant.replay(&family, &context).unwrap();

        let conflict = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[2]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        assert!(conflict.same_path_allocation(&paths[2]));
        assert!(matches!(
            conflict.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::ProvedEmpty(
                ParametricSectorFormulaAffineEmptyReason::ConflictingFixedValues {
                    first_decision_ordinal: 1,
                    second_decision_ordinal: 2,
                    index: 2,
                    first_value: i64::MAX,
                    second_value: 1,
                    ..
                }
            )
        ));
        assert!(conflict.geometry().is_none());
        conflict.replay(&family, &context).unwrap();

        let actionable = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        assert!(actionable.same_path_allocation(&paths[5]));
        assert_eq!(
            actionable.outcome(),
            ParametricSectorFormulaAffineTerminalOutcome::Actionable
        );
        let geometry = actionable.geometry().unwrap();
        assert_eq!(
            geometry.constants(),
            &[Integer::from(1), Integer::from(1), Integer::from(1)]
        );
        assert!(geometry.free_positions().is_empty());
        assert!(geometry.compact_linear_coefficients().is_empty());
        assert_eq!(actionable.guards().len(), 1);
        assert_eq!(actionable.guards()[0].decision_ordinal(), 1);
        assert!(matches!(
            actionable.guards()[0].class(),
            ParametricSectorFormulaAffineGuardClass::DischargedNonzeroIntegerConstant(_)
        ));
        assert!(actionable.guards()[0].class().condition().is_none());
        assert!(execution_guard_aggregate_fits_preflight(
            actionable.stats().guard_execution(),
            actionable.stats().guard_preflight(),
        ));
        actionable.replay(&family, &context).unwrap();
    }

    #[test]
    fn prepared_guard_token_bytes_accept_exact_and_reject_one_below() {
        let (family, context, paths) = generated_sunset_paths("affine-terminal-token-byte-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let token_bytes = observed
            .stats()
            .prepared_guard_token_byte_envelope()
            .max(observed.stats().prepared_guard_token_capacity_bytes());
        assert!(token_bytes > 0);

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_prepared_guard_token_bytes = token_bytes;
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            exact,
        )
        .unwrap();

        let mut below = exact;
        below.max_prepared_guard_token_bytes = token_bytes - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[5]),
                below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal prepared guard token bytes",
                requested: token_bytes,
                limit: token_bytes - 1,
            }
        );
    }

    #[test]
    fn guard_origin_envelope_accepts_exact_and_rejects_one_below() {
        let (family, context, paths) =
            generated_sunset_paths("affine-terminal-origin-envelope-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let origin_bytes = observed.stats().guard_origin_retained_byte_envelope();
        assert!(origin_bytes > 0);
        assert_eq!(observed.stats().total_guard_origin_retained_bytes(), 0);

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_total_guard_origin_retained_bytes = origin_bytes;
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            exact,
        )
        .unwrap();

        let mut below = exact;
        below.max_total_guard_origin_retained_bytes = origin_bytes - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[5]),
                below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal total guard origin retained bytes",
                requested: origin_bytes,
                limit: origin_bytes - 1,
            }
        );
    }

    #[test]
    fn aggregate_guard_source_remainder_is_remapped_to_outer_limit() {
        let (family, context, paths) =
            generated_sunset_paths("affine-terminal-aggregate-source-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[0]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let per_guard_source_terms = observed
            .guards()
            .iter()
            .map(|guard| guard.composition_stats().source_terms())
            .collect::<Vec<_>>();
        assert_eq!(per_guard_source_terms.len(), 3);
        assert!(per_guard_source_terms.iter().all(|&terms| terms > 0));
        let source_terms = per_guard_source_terms.iter().sum::<usize>();
        assert_eq!(
            source_terms,
            observed.stats().guard_preflight().source_terms()
        );

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_total_guard_source_terms = source_terms;
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[0]),
            exact,
        )
        .unwrap();

        let mut below = exact;
        below.max_total_guard_source_terms = source_terms - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[0]),
                below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal total guard source terms",
                requested: source_terms,
                limit: source_terms - 1,
            }
        );
    }

    #[test]
    fn aggregate_guard_native_integer_work_remainder_is_remapped_to_outer_limit() {
        let (family, context, paths) =
            generated_sunset_paths("affine-terminal-aggregate-native-integer-work-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[0]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let native_work = observed
            .stats()
            .guard_preflight()
            .native_integer_bit_work_bound();
        assert!(native_work > 0);
        assert_eq!(
            native_work,
            observed
                .stats()
                .guard_execution()
                .native_integer_bit_work_bound()
        );

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_total_guard_native_integer_bit_work_bound = native_work;
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[0]),
            exact,
        )
        .expect("the exact aggregate native integer-work boundary must pass");

        let mut one_below = exact;
        one_below.max_total_guard_native_integer_bit_work_bound = native_work - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[0]),
                one_below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal total guard native integer-bit work bound",
                requested: native_work,
                limit: native_work - 1,
            }
        );
    }

    #[test]
    fn coordinate_conflict_comparisons_accept_exact_and_reject_one_below() {
        let (family, context, paths) =
            generated_sunset_paths("affine-terminal-coordinate-comparison-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[2]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let comparisons = observed.stats().coordinate_conflict_comparisons();
        assert!(comparisons > 0);

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_coordinate_conflict_comparisons = comparisons;
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[2]),
            exact,
        )
        .unwrap();

        let mut below = exact;
        below.max_coordinate_conflict_comparisons = comparisons - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[2]),
                below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal coordinate-conflict comparisons",
                requested: comparisons,
                limit: comparisons - 1,
            }
        );
    }

    #[test]
    fn cylinder_geometry_bytes_are_admitted_before_allocation() {
        let (family, context, paths) =
            generated_sunset_paths("affine-terminal-cylinder-byte-limits");
        let observed = ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            ParametricSectorFormulaAffineTerminalLimits::default(),
        )
        .unwrap();
        let envelope = observed.stats().cylinder_geometry_byte_envelope();
        let capacity = observed.stats().cylinder_geometry_capacity_bytes();
        assert!(envelope > 0);
        assert!(capacity >= envelope);

        let mut exact = ParametricSectorFormulaAffineTerminalLimits::default();
        exact.max_cylinder_geometry_capacity_bytes = envelope.max(capacity);
        ParametricSectorFormulaAffineTerminalCompiler::compile(
            &family,
            &context,
            Arc::clone(&paths[5]),
            exact,
        )
        .unwrap();

        let mut below = exact;
        below.max_cylinder_geometry_capacity_bytes = envelope - 1;
        assert_eq!(
            ParametricSectorFormulaAffineTerminalCompiler::compile(
                &family,
                &context,
                Arc::clone(&paths[5]),
                below,
            )
            .unwrap_err(),
            ParametricSectorFormulaAffineTerminalError::ResourceLimit {
                resource: "affine-terminal cylinder geometry capacity bytes",
                requested: envelope,
                limit: envelope - 1,
            }
        );
    }
}
