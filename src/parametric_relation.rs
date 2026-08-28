//! Typed sparse relations on the parametric integral lattice.

use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;
use std::mem::size_of;
use std::sync::Arc;

use symbolica::prelude::Integer;

use crate::parametric_coefficient::{
    CoefficientPolynomial, ParametricCoefficientSpecializationPreflight,
    ParametricPolynomialSpecializationPreflight, PartialPolynomialSpecializationStats,
    coefficient_owned_retained_byte_bound, insert_parametric_condition,
    insert_specialized_condition,
};
use crate::{
    BasePolynomial, CoefficientLocation, GuardOrigin, GuardRowId, GuardedParametricCoefficient,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    PartialIndexAssignment, SpecializedNonZeroCondition,
};
use crate::{algebra::Coefficient, algebra::CoefficientContext};

/// Versioned canonical text identity for a complete parametric source row.
///
/// V1 is retained as a legacy persistence identifier. New manifests use V2's
/// typed sparse encoding, which does not require a cap-sized canonical Atom
/// `String` as an intermediate value.
pub const PARAMETRIC_RELATION_MANIFEST_V1_SCHEMA: &str = "rustred-parametric-relation-manifest-v1";
pub const PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA: &str = "rustred-parametric-relation-manifest-v2";
const PARAMETRIC_RELATION_COEFFICIENT_V2_SCHEMA: &str = "rustred-parametric-coefficient-sparse-v1";
const PARAMETRIC_RELATION_POLYNOMIAL_V2_SCHEMA: &str = "rustred-parametric-polynomial-sparse-v1";
pub const PARTIAL_PARAMETRIC_RELATION_SPECIALIZATION_V1_SCHEMA: &str =
    "rustred-partial-parametric-relation-specialization-v1";

/// Aggregate prospective limits for one complete specialization of a sparse
/// `K(n)` relation to a concrete `K` relation.
///
/// [`ParametricArithmeticLimits`] continues to bound each individual native
/// polynomial operation.  The remaining fields are deliberately aggregate:
/// a large relation may otherwise repeat an individually admissible
/// specialization enough times to escape a caller's work or memory budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricConcreteSpecializationLimits {
    pub(crate) arithmetic: ParametricArithmeticLimits,
    pub(crate) max_source_terms: usize,
    pub(crate) max_source_exponent_entries: usize,
    pub(crate) max_output_term_bound: usize,
    pub(crate) max_output_exponent_entry_bound: usize,
    pub(crate) max_power_operation_bound: usize,
    pub(crate) max_integer_bit_work_bound: usize,
    pub(crate) max_normalization_input_term_pair_bound: usize,
    pub(crate) max_key_component_bound: usize,
    pub(crate) max_guard_occurrence_bound: usize,
    pub(crate) max_guard_polynomial_retained_byte_bound: usize,
    pub(crate) max_guard_origin_occurrence_bound: usize,
    pub(crate) max_guard_origin_retained_byte_bound: usize,
    pub(crate) max_normalized_coefficient_term_bound: usize,
    pub(crate) max_normalized_coefficient_retained_byte_bound: usize,
    pub(crate) max_concrete_relation_retained_byte_bound: usize,
    pub(crate) max_peak_execution_retained_byte_bound: usize,
}

impl Default for ParametricConcreteSpecializationLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_source_terms: usize::MAX,
            max_source_exponent_entries: usize::MAX,
            max_output_term_bound: usize::MAX,
            max_output_exponent_entry_bound: usize::MAX,
            max_power_operation_bound: usize::MAX,
            max_integer_bit_work_bound: usize::MAX,
            max_normalization_input_term_pair_bound: usize::MAX,
            max_key_component_bound: usize::MAX,
            max_guard_occurrence_bound: usize::MAX,
            max_guard_polynomial_retained_byte_bound: usize::MAX,
            max_guard_origin_occurrence_bound: usize::MAX,
            max_guard_origin_retained_byte_bound: usize::MAX,
            max_normalized_coefficient_term_bound: usize::MAX,
            max_normalized_coefficient_retained_byte_bound: usize::MAX,
            max_concrete_relation_retained_byte_bound: usize::MAX,
            max_peak_execution_retained_byte_bound: usize::MAX,
        }
    }
}

/// Allocation-free census admitted before the first concrete specialization
/// allocation.  Every retained-byte field is a conservative upper bound; it
/// intentionally makes no claim about Symbolica's private native workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricConcreteSpecializationPreflight {
    source_terms: usize,
    source_exponent_entries: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pair_bound: usize,
    key_component_bound: usize,
    guard_occurrence_bound: usize,
    guard_polynomial_retained_byte_bound: usize,
    guard_origin_occurrence_bound: usize,
    guard_origin_retained_byte_bound: usize,
    normalized_coefficient_term_bound: usize,
    normalized_coefficient_retained_byte_bound: usize,
    concrete_relation_retained_byte_bound: usize,
    peak_execution_retained_byte_bound: usize,
}

macro_rules! concrete_specialization_preflight_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricConcreteSpecializationPreflight {
    concrete_specialization_preflight_getters!(
        source_terms,
        source_exponent_entries,
        output_term_bound,
        output_exponent_entry_bound,
        power_operation_bound,
        integer_bit_work_bound,
        normalization_input_term_pair_bound,
        key_component_bound,
        guard_occurrence_bound,
        guard_polynomial_retained_byte_bound,
        guard_origin_occurrence_bound,
        guard_origin_retained_byte_bound,
        normalized_coefficient_term_bound,
        normalized_coefficient_retained_byte_bound,
        concrete_relation_retained_byte_bound,
        peak_execution_retained_byte_bound,
    );
}

/// One immutable, allocation-free authorization to execute an already
/// admitted concrete relation specialization.
///
/// The borrowed relation, conditions, and assignment are deliberately absent
/// from `Debug`; callers can retain and report the numeric census without
/// publishing a proof-private source row or condition polynomial.
pub(crate) struct ParametricConcreteSpecializationPlan<'a> {
    relation: &'a ParametricRelation,
    context: &'a ParametricCoefficientContext,
    assignment: &'a [i64],
    additional_nonzero_conditions: &'a [ParametricNonZeroCondition],
    limits: ParametricConcreteSpecializationLimits,
    preflight: ParametricConcreteSpecializationPreflight,
}

impl fmt::Debug for ParametricConcreteSpecializationPlan<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParametricConcreteSpecializationPlan")
            .field("preflight", &self.preflight)
            .field("private_payload", &"<redacted>")
            .finish()
    }
}

impl ParametricConcreteSpecializationPlan<'_> {
    pub(crate) const fn preflight(&self) -> ParametricConcreteSpecializationPreflight {
        self.preflight
    }

    /// Consume the authorization and execute the existing specialization
    /// backend.  The observed durable relation must remain inside the bound
    /// established before this call was possible.
    pub(crate) fn execute(self) -> Result<ConcreteRelation, ParametricRelationError> {
        let concrete = self
            .relation
            .specialize_with_additional_nonzero_conditions(
                self.context,
                self.assignment,
                self.additional_nonzero_conditions.iter(),
                self.limits.arithmetic,
            )?;
        let observed = concrete.owned_retained_byte_bound().ok_or(
            ParametricRelationError::ResourceCountOverflow {
                resource: "concrete specialization observed retained bytes",
            },
        )?;
        relation_check_limit(
            "concrete specialization observed retained bytes",
            observed,
            self.preflight.concrete_relation_retained_byte_bound,
        )?;
        Ok(concrete)
    }
}

/// Aggregate prospective bounds for the split recentering used by a
/// residual-affine pivot.  Unlike [`ParametricRelation::translated`], this
/// operation applies one displacement to coefficient/guard variables and a
/// different center to integral keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricAffineFreeRecenteringLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_terms: usize,
    pub max_guards: usize,
    pub max_translation_components: usize,
    pub max_key_subtraction_boundary_checks: usize,
    pub max_source_terms: usize,
    pub max_source_exponent_entries: usize,
    pub max_output_terms: usize,
    pub max_output_exponent_entries: usize,
    pub max_power_operations: usize,
    pub max_integer_bit_work: usize,
    pub max_normalized_coefficient_terms: usize,
    pub max_retained_bytes: usize,
}

impl Default for ParametricAffineFreeRecenteringLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_terms: 16_000_000,
            max_guards: 16_000_000,
            max_translation_components: 16_000_000,
            max_key_subtraction_boundary_checks: 64_000_000_000,
            max_source_terms: 64_000_000_000,
            max_source_exponent_entries: 4_000_000_000_000_000_000,
            max_output_terms: 64_000_000_000,
            max_output_exponent_entries: 4_000_000_000_000_000_000,
            max_power_operations: 4_000_000_000_000_000_000,
            max_integer_bit_work: 4_000_000_000_000_000_000,
            max_normalized_coefficient_terms: 64_000_000_000,
            max_retained_bytes: 64 * 1024 * 1024 * 1024,
        }
    }
}

/// Reproducible prospective census for one successful split recentering.
/// The retained-byte figure is a conservative pre-execution envelope, not a
/// claim about Symbolica's transient workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricAffineFreeRecenteringStats {
    terms: usize,
    guards: usize,
    translation_components: usize,
    key_subtraction_boundary_checks: usize,
    source_terms: usize,
    source_exponent_entries: usize,
    output_terms: usize,
    output_exponent_entries: usize,
    power_operations: usize,
    integer_bit_work: usize,
    normalized_coefficient_terms: usize,
    retained_bytes: usize,
}

impl ParametricAffineFreeRecenteringStats {
    pub(crate) const fn terms(self) -> usize {
        self.terms
    }
    pub(crate) const fn guards(self) -> usize {
        self.guards
    }
    pub(crate) const fn translation_components(self) -> usize {
        self.translation_components
    }
    pub(crate) const fn key_subtraction_boundary_checks(self) -> usize {
        self.key_subtraction_boundary_checks
    }
    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }
    pub(crate) const fn source_exponent_entries(self) -> usize {
        self.source_exponent_entries
    }
    pub(crate) const fn output_terms(self) -> usize {
        self.output_terms
    }
    pub(crate) const fn output_exponent_entries(self) -> usize {
        self.output_exponent_entries
    }
    pub(crate) const fn power_operations(self) -> usize {
        self.power_operations
    }
    pub(crate) const fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }
    pub(crate) const fn normalized_coefficient_terms(self) -> usize {
        self.normalized_coefficient_terms
    }
    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// Aggregate limits for one equality-locus-bound source-row specialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartialParametricRelationSpecializationLimits {
    pub arithmetic: ParametricArithmeticLimits,
    pub max_assignments: usize,
    pub max_terms: usize,
    pub max_guards: usize,
    pub max_base_assumptions: usize,
    pub max_source_terms: usize,
    pub max_output_terms: usize,
    pub max_power_operations: usize,
    pub max_integer_bit_work: usize,
    pub max_retained_terms: usize,
    pub max_retained_bytes: usize,
}

impl Default for PartialParametricRelationSpecializationLimits {
    fn default() -> Self {
        Self {
            arithmetic: ParametricArithmeticLimits::default(),
            max_assignments: 1_000_000,
            max_terms: 4_000_000,
            max_guards: 4_000_000,
            max_base_assumptions: 4_000_000,
            max_source_terms: 16_000_000,
            max_output_terms: 16_000_000,
            max_power_operations: 64_000_000,
            max_integer_bit_work: 64_000_000,
            max_retained_terms: 32_000_000,
            max_retained_bytes: 2 * 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartialParametricRelationSpecializationStats {
    terms: usize,
    guards: usize,
    base_assumptions: usize,
    source_terms: usize,
    output_terms: usize,
    power_operations: usize,
    integer_bit_work: usize,
    retained_terms: usize,
    retained_bytes: usize,
}

impl PartialParametricRelationSpecializationStats {
    pub fn terms(self) -> usize {
        self.terms
    }
    pub fn guards(self) -> usize {
        self.guards
    }
    pub fn base_assumptions(self) -> usize {
        self.base_assumptions
    }
    pub fn source_terms(self) -> usize {
        self.source_terms
    }
    pub fn output_terms(self) -> usize {
        self.output_terms
    }
    pub fn power_operations(self) -> usize {
        self.power_operations
    }
    pub fn integer_bit_work(self) -> usize {
        self.integer_bit_work
    }
    pub fn retained_terms(self) -> usize {
        self.retained_terms
    }
    pub fn retained_bytes(self) -> usize {
        self.retained_bytes
    }
}

/// One base-field nonzero assumption retained after sparse index
/// specialization.  Such a condition is constant in the remaining index
/// field and therefore must not become an index-case branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartialSpecializationBaseAssumption {
    condition: ParametricNonZeroCondition,
}

impl PartialSpecializationBaseAssumption {
    pub fn condition(&self) -> &ParametricNonZeroCondition {
        &self.condition
    }
}

/// An exact source-row identity valid only on [`Self::assignment`].
///
/// The rebuilt relation is intentionally not public: treating it as a global
/// `K(n)` identity would be unsound.  The future exceptional-leaf driver may
/// borrow it only through the crate-private bound re-elimination accessor.
#[derive(Clone, Debug)]
pub struct PartialParametricRelationSpecialization {
    schema: &'static str,
    source: Arc<ParametricRelation>,
    assignment: PartialIndexAssignment,
    relation: ParametricRelation,
    base_assumptions: Arc<Vec<PartialSpecializationBaseAssumption>>,
    limits: PartialParametricRelationSpecializationLimits,
    stats: PartialParametricRelationSpecializationStats,
}

impl PartialParametricRelationSpecialization {
    pub fn schema(&self) -> &'static str {
        self.schema
    }
    pub fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }
    pub fn base_assumptions(&self) -> &[PartialSpecializationBaseAssumption] {
        &self.base_assumptions
    }
    pub fn stats(&self) -> PartialParametricRelationSpecializationStats {
        self.stats
    }

    pub fn replay(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        if self.schema != PARTIAL_PARAMETRIC_RELATION_SPECIALIZATION_V1_SCHEMA {
            return Err(ParametricRelationError::SpecializationReplayMismatch);
        }
        let replayed =
            self.source
                .partially_specialized_on(context, self.assignment.clone(), self.limits)?;
        if replayed.assignment == self.assignment
            && replayed
                .relation
                .has_identical_guard_provenance(&self.relation)
            && replayed.base_assumptions == self.base_assumptions
            && replayed.stats == self.stats
        {
            Ok(())
        } else {
            Err(ParametricRelationError::SpecializationReplayMismatch)
        }
    }

    pub(crate) fn relation_for_bound_reelimination(&self) -> &ParametricRelation {
        &self.relation
    }
}

/// A checked displacement in one family's integral-index lattice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IndexShift(Vec<i64>);

impl IndexShift {
    pub fn try_new(
        values: impl IntoIterator<Item = i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        let mut retained = Vec::new();
        try_reserve_relation_entries("index-shift components", &mut retained, expected_arity)?;
        let mut values = values.into_iter();
        while retained.len() < expected_arity {
            let Some(value) = values.next() else {
                return Err(ParametricRelationError::WrongArity {
                    expected: expected_arity,
                    actual: retained.len(),
                });
            };
            retained.push(value);
        }
        if values.next().is_some() {
            let actual = expected_arity.checked_add(1).ok_or(
                ParametricRelationError::ResourceCountOverflow {
                    resource: "index-shift components",
                },
            )?;
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual,
            });
        }
        Self::try_from_preallocated(retained, expected_arity)
    }

    /// Retain an allocation which its caller already acquired fallibly.
    /// This is crate-private so public construction semantics remain intact.
    pub(crate) fn try_from_preallocated(
        values: Vec<i64>,
        expected_arity: usize,
    ) -> Result<Self, ParametricRelationError> {
        if values.len() != expected_arity {
            return Err(ParametricRelationError::WrongArity {
                expected: expected_arity,
                actual: values.len(),
            });
        }
        Ok(Self(values))
    }

    pub fn values(&self) -> &[i64] {
        &self.0
    }

    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        self.0.capacity().checked_mul(size_of::<i64>())
    }

    pub fn arity(&self) -> usize {
        self.0.len()
    }

    pub fn checked_add(&self, other: &Self) -> Result<Self, ParametricRelationError> {
        if self.arity() != other.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity(),
                actual: other.arity(),
            });
        }
        let mut values = Vec::new();
        try_reserve_relation_entries("summed index-shift components", &mut values, self.arity())?;
        for (position, (&left, &right)) in self.0.iter().zip(&other.0).enumerate() {
            values.push(
                left.checked_add(right)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Ok(Self(values))
    }
}

/// Constructs arity-authenticated shifts without repeating length checks at
/// every generator call site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexSpace {
    arity: usize,
}

impl IndexSpace {
    pub fn try_new(arity: usize) -> Result<Self, ParametricRelationError> {
        if arity == 0 {
            Err(ParametricRelationError::EmptyIndexSpace)
        } else {
            Ok(Self { arity })
        }
    }

    pub fn arity(self) -> usize {
        self.arity
    }

    /// Fallible zero-shift construction for resource-hardened callers.
    pub fn try_zero(self) -> Result<IndexShift, ParametricRelationError> {
        let mut values = Vec::new();
        try_reserve_relation_entries("zero index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        Ok(IndexShift(values))
    }

    /// Compatibility wrapper. New bounded code should use [`Self::try_zero`].
    pub fn zero(self) -> IndexShift {
        self.try_zero()
            .expect("allocating an IndexSpace zero shift failed")
    }

    pub fn unit(
        self,
        position: usize,
        direction: i64,
    ) -> Result<IndexShift, ParametricRelationError> {
        if position >= self.arity {
            return Err(ParametricRelationError::IndexOutOfRange {
                position,
                arity: self.arity,
            });
        }
        let mut values = Vec::new();
        try_reserve_relation_entries("unit index-shift components", &mut values, self.arity)?;
        values.resize(self.arity, 0);
        values[position] = direction;
        Ok(IndexShift(values))
    }

    pub fn shift(
        self,
        values: impl IntoIterator<Item = i64>,
    ) -> Result<IndexShift, ParametricRelationError> {
        IndexShift::try_new(values, self.arity)
    }
}

/// Stable source identity of a generated relation.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ParametricRowId {
    OrdinaryIbp {
        /// Loops first, then external momenta.
        contraction_momentum: usize,
        differentiated_loop: usize,
    },
    LorentzInvariance {
        first_external: usize,
        second_external: usize,
    },
    Derived {
        label: Arc<str>,
    },
}

impl ParametricRowId {
    pub fn guard_identity(&self) -> GuardRowId {
        match self {
            Self::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            } => GuardRowId::OrdinaryIbp {
                contraction_momentum: *contraction_momentum,
                differentiated_loop: *differentiated_loop,
            },
            Self::LorentzInvariance {
                first_external,
                second_external,
            } => GuardRowId::LorentzInvariance {
                first_external: *first_external,
                second_external: *second_external,
            },
            Self::Derived { label } => GuardRowId::Derived {
                label: label.clone(),
            },
        }
    }

    /// Version-stable identity used inside persisted proof manifests.
    pub fn stable_string(&self) -> String {
        self.guard_identity().stable_string()
    }

    fn write_stable(&self, writer: &mut impl fmt::Write) -> fmt::Result {
        self.guard_identity().write_stable(writer)
    }
}

/// A raw parametric zero equation together with every condition inherited
/// before fraction-field cancellation.
#[derive(Clone, Debug)]
pub struct ParametricRelation {
    family_fingerprint: Arc<str>,
    context_fingerprint: Arc<str>,
    row_id: ParametricRowId,
    arity: usize,
    terms: BTreeMap<IndexShift, ParametricCoefficient>,
    // Compatibility polynomial-only view.  `guarded_nonzero` is canonical.
    nonzero: Vec<ParametricPolynomial>,
    guarded_nonzero: Vec<ParametricNonZeroCondition>,
}

// Preserve the pre-provenance meaning of relation equality.  Provenance is
// auditable through `guarded_nonzero_conditions`, but adapter history does not
// change the mathematical sparse relation or its exceptional polynomial set.
impl PartialEq for ParametricRelation {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.context_fingerprint == other.context_fingerprint
            && self.row_id == other.row_id
            && self.arity == other.arity
            && self.terms == other.terms
            && self.nonzero == other.nonzero
    }
}

impl Eq for ParametricRelation {}

struct RelationManifestBuilder {
    output: String,
    expected_bytes: usize,
    error: Option<ParametricRelationError>,
}

struct ManifestByteCounter {
    bytes: usize,
    byte_offset: usize,
    max_bytes: usize,
    error: Option<ParametricRelationError>,
}

impl ManifestByteCounter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: 0,
            byte_offset: 0,
            max_bytes,
            error: None,
        }
    }
}

impl fmt::Write for ManifestByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(local_requested) = self.bytes.checked_add(value.len()) else {
            self.error = Some(ParametricRelationError::ResourceCountOverflow {
                resource: "parametric relation manifest bytes",
            });
            return Err(fmt::Error);
        };
        let Some(requested) = self.byte_offset.checked_add(local_requested) else {
            self.error = Some(ParametricRelationError::ResourceCountOverflow {
                resource: "parametric relation manifest bytes",
            });
            return Err(fmt::Error);
        };
        if requested > self.max_bytes {
            self.error = Some(ParametricRelationError::ResourceLimit {
                resource: "parametric relation manifest bytes",
                requested,
                limit: self.max_bytes,
            });
            return Err(fmt::Error);
        }
        self.bytes = local_requested;
        Ok(())
    }
}

impl RelationManifestBuilder {
    fn try_new(expected_bytes: usize) -> Result<Self, ParametricRelationError> {
        let mut output = String::new();
        output.try_reserve_exact(expected_bytes).map_err(|_| {
            ParametricRelationError::AllocationFailure {
                resource: "parametric relation manifest bytes",
                requested: expected_bytes,
            }
        })?;
        Ok(Self {
            output,
            expected_bytes,
            error: None,
        })
    }

    fn finish(self) -> String {
        self.output
    }
}

impl fmt::Write for RelationManifestBuilder {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.output.len().checked_add(value.len()) else {
            self.error = Some(ParametricRelationError::ResourceCountOverflow {
                resource: "parametric relation manifest bytes",
            });
            return Err(fmt::Error);
        };
        if requested > self.expected_bytes {
            self.error = Some(ParametricRelationError::ResourceCountOverflow {
                resource: "parametric relation manifest bytes",
            });
            return Err(fmt::Error);
        }
        // `try_new` reserved the complete exact output before rendering, so
        // this append cannot trigger a second allocation.
        self.output.push_str(value);
        Ok(())
    }
}

enum RelationManifestSink<'a> {
    Counter(&'a mut ManifestByteCounter),
    Output(&'a mut RelationManifestBuilder),
}

impl RelationManifestSink<'_> {
    fn take_error(&mut self) -> ParametricRelationError {
        match self {
            Self::Counter(counter) => counter.error.take(),
            Self::Output(output) => output.error.take(),
        }
        .unwrap_or(ParametricRelationError::ResourceCountOverflow {
            resource: "parametric relation manifest bytes",
        })
    }

    fn finish_write(&mut self, result: fmt::Result) -> Result<(), ParametricRelationError> {
        result.map_err(|_| self.take_error())
    }
}

impl fmt::Write for RelationManifestSink<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self {
            Self::Counter(counter) => counter.write_str(value),
            Self::Output(output) => output.write_str(value),
        }
    }
}

/// Typed semantic events emitted while the canonical V2 grammar is streamed.
///
/// Exact structural identities implement this observer to census every nested
/// payload in one semantic-observer traversal. Canonical length-prefix sizing
/// performs that semantic traversal during its allocation-free physical sizing
/// subpass, then emits the payload with a no-op observer. The default methods
/// make the standalone persistence writer a zero-cost no-op observer.
pub(crate) trait ParametricRelationV2Observer {
    /// Finite exact-identity observers override this to bound every physical
    /// length-prefix sizing subpass. Standalone persistence keeps its outer
    /// manifest counter as the byte-limit authority.
    fn length_prefix_byte_limit(&self) -> usize {
        usize::MAX
    }

    /// Preserve the embedding writer's resource vocabulary when a nested
    /// length-only subpass reaches its finite ceiling.
    fn observe_length_prefix_limit_exceeded(
        &mut self,
        _local_requested: usize,
        _local_limit: usize,
    ) -> fmt::Result {
        Err(fmt::Error)
    }

    fn observe_text_payload(&mut self, _bytes: usize) -> fmt::Result {
        Ok(())
    }

    fn observe_unsigned(&mut self, _value: u128) -> fmt::Result {
        Ok(())
    }

    fn observe_signed_i64(&mut self, _value: i64) -> fmt::Result {
        Ok(())
    }

    fn observe_integer(&mut self, _value: &Integer) -> fmt::Result {
        Ok(())
    }

    fn observe_polynomial(&mut self, _polynomial: &CoefficientPolynomial) -> fmt::Result {
        Ok(())
    }
}

struct IgnoreParametricRelationV2Events;

impl ParametricRelationV2Observer for IgnoreParametricRelationV2Events {}

trait RelationManifestPayload {
    /// Whether this payload is one stable textual value, rather than a typed
    /// sparse structure whose internal resources are observed separately.
    const TEXTUAL: bool;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result;
}

impl RelationManifestPayload for str {
    const TEXTUAL: bool = true;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        _observer: &mut O,
    ) -> fmt::Result {
        writer.write_str(self)
    }
}

impl RelationManifestPayload for ParametricRowId {
    const TEXTUAL: bool = true;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result {
        observe_parametric_row_id(observer, self)?;
        self.write_stable(writer)
    }
}

impl RelationManifestPayload for IndexShift {
    const TEXTUAL: bool = false;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result {
        observe_usize(observer, self.arity())?;
        for (ordinal, value) in self.values().iter().enumerate() {
            if ordinal != 0 {
                writer.write_str(",")?;
            }
            observer.observe_signed_i64(*value)?;
            write!(writer, "{value}")?;
        }
        Ok(())
    }
}

impl RelationManifestPayload for Coefficient {
    const TEXTUAL: bool = false;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result {
        write_typed_coefficient_observed(writer, self, observer)
    }
}

impl RelationManifestPayload for CoefficientPolynomial {
    const TEXTUAL: bool = false;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result {
        write_typed_polynomial_observed(writer, self, observer)
    }
}

impl RelationManifestPayload for GuardOrigin {
    const TEXTUAL: bool = true;

    fn write_manifest_payload<W: fmt::Write, O: ParametricRelationV2Observer>(
        &self,
        writer: &mut W,
        observer: &mut O,
    ) -> fmt::Result {
        observe_guard_origin(observer, self)?;
        self.write_stable(writer)
    }
}

fn write_length_prefixed_manifest_payload<
    W: fmt::Write,
    O: ParametricRelationV2Observer,
    T: RelationManifestPayload + ?Sized,
>(
    writer: &mut W,
    value: &T,
    observer: &mut O,
) -> fmt::Result {
    // This allocation-free physical serialization subpass computes the byte
    // prefix without retaining a nested String. This sizing pass owns the one
    // semantic-observer traversal, allowing semantic ceilings to reject before
    // deep payload formatting. Exact-identity observers also propagate their
    // finite remaining byte ceiling here.
    let length_prefix_limit = observer.length_prefix_byte_limit();
    let mut counter = ManifestByteCounter::new(length_prefix_limit);
    if value
        .write_manifest_payload(&mut counter, observer)
        .is_err()
    {
        if let Some(ParametricRelationError::ResourceLimit {
            requested, limit, ..
        }) = counter.error
        {
            observer.observe_length_prefix_limit_exceeded(requested, limit)?;
        }
        return Err(fmt::Error);
    }
    observe_usize(observer, counter.bytes)?;
    if T::TEXTUAL {
        observer.observe_text_payload(counter.bytes)?;
    }
    write!(writer, "{}:", counter.bytes)?;
    value.write_manifest_payload(writer, &mut IgnoreParametricRelationV2Events)
}

fn observe_usize<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    value: usize,
) -> fmt::Result {
    observer.observe_unsigned(value as u128)
}

fn observe_u64<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    value: u64,
) -> fmt::Result {
    observer.observe_unsigned(u128::from(value))
}

fn observe_i64_slice<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    values: &[i64],
) -> fmt::Result {
    observe_usize(observer, values.len())?;
    for value in values {
        observer.observe_signed_i64(*value)?;
    }
    Ok(())
}

fn observe_usize_slice<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    values: &[usize],
) -> fmt::Result {
    observe_usize(observer, values.len())?;
    for value in values {
        observe_usize(observer, *value)?;
    }
    Ok(())
}

fn observe_parametric_row_id<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    row: &ParametricRowId,
) -> fmt::Result {
    match row {
        ParametricRowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => {
            observe_usize(observer, *contraction_momentum)?;
            observe_usize(observer, *differentiated_loop)
        }
        ParametricRowId::LorentzInvariance {
            first_external,
            second_external,
        } => {
            observe_usize(observer, *first_external)?;
            observe_usize(observer, *second_external)
        }
        ParametricRowId::Derived { label } => observe_usize(observer, label.len()),
    }
}

fn observe_guard_row_id<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    row: &GuardRowId,
) -> fmt::Result {
    match row {
        GuardRowId::OrdinaryIbp {
            contraction_momentum,
            differentiated_loop,
        } => {
            observe_usize(observer, *contraction_momentum)?;
            observe_usize(observer, *differentiated_loop)
        }
        GuardRowId::LorentzInvariance {
            first_external,
            second_external,
        } => {
            observe_usize(observer, *first_external)?;
            observe_usize(observer, *second_external)
        }
        GuardRowId::Derived { label } => observe_usize(observer, label.len()),
    }
}

fn observe_coefficient_location<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    location: &CoefficientLocation,
) -> fmt::Result {
    match location {
        CoefficientLocation::Dimension | CoefficientLocation::BasisDeterminantNumerator => Ok(()),
        CoefficientLocation::DenominatorConstant { denominator }
        | CoefficientLocation::PowerShift { denominator } => observe_usize(observer, *denominator),
        CoefficientLocation::DenominatorCoefficient {
            denominator,
            coordinate,
        } => {
            observe_usize(observer, *denominator)?;
            observe_usize(observer, *coordinate)
        }
        CoefficientLocation::ExternalGram { row, column } => {
            observe_usize(observer, *row)?;
            observe_usize(observer, *column)
        }
    }
}

/// Exhaustive semantic mirror of `GuardOrigin::write_stable` payload fields.
/// Patterns deliberately avoid `..`: adding a provenance field or variant
/// must update this census before the crate can compile. The complete rendered
/// origin is also charged as one exact textual payload by the shared length-
/// prefix writer, so punctuation and future fixed variant names cannot evade
/// the string-byte limit.
fn observe_guard_origin<O: ParametricRelationV2Observer + ?Sized>(
    observer: &mut O,
    origin: &GuardOrigin,
) -> fmt::Result {
    match origin {
        GuardOrigin::FamilyInputCoefficientDenominator { location } => {
            observe_coefficient_location(observer, location)
        }
        GuardOrigin::FamilyBasisDeterminantNumerator
        | GuardOrigin::GuardedDivisionDividendDenominator
        | GuardOrigin::GuardedDivisionDivisorDenominator
        | GuardOrigin::GuardedDivisionDivisorNumerator
        | GuardOrigin::ExplicitRelationCondition
        | GuardOrigin::GeneratedAffineSealedCondition
        | GuardOrigin::CoefficientSpecializationDenominator
        | GuardOrigin::CoefficientPartialSpecializationDenominator
        | GuardOrigin::QuotientPivotNumerator
        | GuardOrigin::ExplicitShiftOperatorCondition => Ok(()),
        GuardOrigin::PowerShiftSupport { denominator } => observe_usize(observer, *denominator),
        GuardOrigin::RelationConditionAttached { row }
        | GuardOrigin::ShiftOperatorConditionAttached { row }
        | GuardOrigin::ShiftOperatorInputTermDenominator { row }
        | GuardOrigin::ShiftOperatorCollectedTermDenominator { row }
        | GuardOrigin::ShiftOperatorFromRelationAdapter { row }
        | GuardOrigin::ShiftOperatorToRelationAdapter { row } => {
            observe_guard_row_id(observer, row)
        }
        GuardOrigin::RelationInputTermDenominator { row, shift }
        | GuardOrigin::RelationCollectedTermDenominator { row, shift }
        | GuardOrigin::RelationPartialSpecializationTermDenominator { row, shift } => {
            observe_guard_row_id(observer, row)?;
            observe_i64_slice(observer, shift)
        }
        GuardOrigin::RelationScaleFactorDenominator {
            target_row,
            source_row,
        } => {
            observe_guard_row_id(observer, target_row)?;
            observe_guard_row_id(observer, source_row)
        }
        GuardOrigin::RelationTranslation {
            source_row,
            target_row,
            offset,
        } => {
            observe_guard_row_id(observer, source_row)?;
            observe_guard_row_id(observer, target_row)?;
            observe_i64_slice(observer, offset)
        }
        GuardOrigin::RelationAffineFreeRecentering {
            source_row,
            target_row,
            coefficient_offset,
            key_center,
        } => {
            observe_guard_row_id(observer, source_row)?;
            observe_guard_row_id(observer, target_row)?;
            observe_i64_slice(observer, coefficient_offset)?;
            observe_i64_slice(observer, key_center)
        }
        GuardOrigin::RelationIndexPermutation {
            source_row,
            target_row,
            source_to_target,
        } => {
            observe_guard_row_id(observer, source_row)?;
            observe_guard_row_id(observer, target_row)?;
            observe_usize_slice(observer, source_to_target)
        }
        GuardOrigin::IndexTranslation { offset } => observe_i64_slice(observer, offset),
        GuardOrigin::IndexPermutation { source_to_target } => {
            observe_usize_slice(observer, source_to_target)
        }
        GuardOrigin::VerifiedSymmetryMapDomain {
            source_to_target,
            condition_ordinal,
        } => {
            observe_usize_slice(observer, source_to_target)?;
            observe_usize(observer, *condition_ordinal)
        }
        GuardOrigin::IndexSpecialization { assignment } => observe_i64_slice(observer, assignment),
        GuardOrigin::PartialIndexSpecialization { assignments } => {
            observe_usize(observer, assignments.len())?;
            for (position, value) in assignments {
                observe_usize(observer, *position)?;
                observer.observe_signed_i64(*value)?;
            }
            Ok(())
        }
        GuardOrigin::ResidualAffineBranchNonzeroGuardSubstitution {
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
            structural_locus_ordinal,
        } => {
            observe_u64(observer, *source_case)?;
            observe_usize(observer, *source_work_item_ordinal)?;
            observe_usize(observer, *ready_terminal_ordinal)?;
            observe_usize(observer, *structural_locus_ordinal)
        }
        GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
            row,
            shift,
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
        } => {
            observe_guard_row_id(observer, row)?;
            observe_i64_slice(observer, shift)?;
            observe_u64(observer, *source_case)?;
            observe_usize(observer, *source_work_item_ordinal)?;
            observe_usize(observer, *ready_terminal_ordinal)
        }
        GuardOrigin::RelationResidualAffineBranchSubstitution {
            source_row,
            target_row,
            source_case,
            source_work_item_ordinal,
            ready_terminal_ordinal,
        } => {
            observe_guard_row_id(observer, source_row)?;
            observe_guard_row_id(observer, target_row)?;
            observe_u64(observer, *source_case)?;
            observe_usize(observer, *source_work_item_ordinal)?;
            observe_usize(observer, *ready_terminal_ordinal)
        }
        GuardOrigin::ConcreteQuotientEliminationPivotNumerator { pivot } => {
            observe_usize(observer, *pivot)
        }
        GuardOrigin::GeneratedAffineGroupRecentering {
            solve_group_ordinal,
            database_epoch,
            event_ordinal,
        } => {
            observe_usize(observer, *solve_group_ordinal)?;
            observe_usize(observer, *database_epoch)?;
            observe_usize(observer, *event_ordinal)
        }
        GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal,
            database_epoch,
            event_ordinal,
            operation_ordinal,
            term_ordinal,
            pivot_normalization,
        } => {
            observe_usize(observer, *solve_group_ordinal)?;
            observe_usize(observer, *database_epoch)?;
            observe_usize(observer, *event_ordinal)?;
            observe_usize(observer, *operation_ordinal)?;
            observe_usize(observer, *term_ordinal)?;
            observe_usize(observer, usize::from(*pivot_normalization))
        }
    }
}

fn write_typed_integer<W: fmt::Write + ?Sized>(writer: &mut W, value: &Integer) -> fmt::Result {
    match value {
        Integer::Single(value) => {
            writer.write_char(if *value < 0 { '-' } else { '+' })?;
            write!(writer, "{:X}", value.unsigned_abs())
        }
        Integer::Double(value) => {
            writer.write_char(if *value < 0 { '-' } else { '+' })?;
            write!(writer, "{:X}", value.unsigned_abs())
        }
        Integer::Large(value) => {
            writer.write_char(if value.is_negative() { '-' } else { '+' })?;
            // `as_limbs` borrows GMP's least-significant-first magnitude.
            // Stream it most-significant-first, padding every lower limb, so
            // the resulting hexadecimal magnitude is independent of whether
            // GMP was built with 32- or 64-bit limbs and needs no large
            // formatter-owned String.
            let limbs = value.as_limbs();
            let Some((most_significant, lower_limbs)) = limbs.split_last() else {
                return writer.write_char('0');
            };
            write!(writer, "{most_significant:X}")?;
            let limb_hex_digits = std::mem::size_of_val(most_significant) * 2;
            for limb in lower_limbs.iter().rev() {
                write!(writer, "{limb:0limb_hex_digits$X}")?;
            }
            Ok(())
        }
    }
}

pub(crate) fn write_typed_polynomial<W: fmt::Write + ?Sized>(
    writer: &mut W,
    polynomial: &CoefficientPolynomial,
) -> fmt::Result {
    write_typed_polynomial_observed(writer, polynomial, &mut IgnoreParametricRelationV2Events)
}

fn write_typed_polynomial_observed<
    W: fmt::Write + ?Sized,
    O: ParametricRelationV2Observer + ?Sized,
>(
    writer: &mut W,
    polynomial: &CoefficientPolynomial,
    observer: &mut O,
) -> fmt::Result {
    observer.observe_polynomial(polynomial)?;
    observe_usize(observer, polynomial.variables.len())?;
    observe_usize(observer, polynomial.nterms())?;
    write!(
        writer,
        "{PARAMETRIC_RELATION_POLYNOMIAL_V2_SCHEMA}|variables={}|terms={}",
        polynomial.variables.len(),
        polynomial.nterms()
    )?;
    for (term, coefficient) in polynomial.coefficients.iter().enumerate() {
        observer.observe_integer(coefficient)?;
        writer.write_str("|coefficient=")?;
        write_typed_integer(writer, coefficient)?;
        writer.write_str("|exponents=")?;
        let exponents = polynomial.exponents(term);
        observe_usize(observer, exponents.len())?;
        for (ordinal, exponent) in exponents.iter().enumerate() {
            if ordinal != 0 {
                writer.write_char(',')?;
            }
            observe_usize(observer, usize::from(*exponent))?;
            write!(writer, "{exponent}")?;
        }
    }
    Ok(())
}

fn write_typed_coefficient<W: fmt::Write + ?Sized>(
    writer: &mut W,
    coefficient: &Coefficient,
) -> fmt::Result {
    write_typed_coefficient_observed(writer, coefficient, &mut IgnoreParametricRelationV2Events)
}

fn write_typed_coefficient_observed<
    W: fmt::Write + ?Sized,
    O: ParametricRelationV2Observer + ?Sized,
>(
    writer: &mut W,
    coefficient: &Coefficient,
    observer: &mut O,
) -> fmt::Result {
    writer.write_str(PARAMETRIC_RELATION_COEFFICIENT_V2_SCHEMA)?;
    writer.write_str("|numerator=")?;
    write_typed_polynomial_observed(writer, &coefficient.numerator, observer)?;
    writer.write_str("|denominator=")?;
    write_typed_polynomial_observed(writer, &coefficient.denominator, observer)
}

/// Stream the complete canonical V2 relation manifest into an arbitrary
/// checked writer without retaining any intermediate manifest `String`.
///
/// This is the single grammar authority shared by persisted relation
/// manifests and larger exact structural identities.
pub(crate) fn write_relation_manifest_v2<W: fmt::Write>(
    writer: &mut W,
    relation: &ParametricRelation,
) -> fmt::Result {
    write_relation_manifest_v2_observed(writer, relation, &mut IgnoreParametricRelationV2Events)
}

/// Stream V2 bytes with one complete semantic-observer traversal.
///
/// This is not one physical serialization traversal: allocation-free
/// length-only subpasses own the semantic observation, then nested payloads are
/// emitted with the no-op observer. Consequently every semantic event is
/// observed exactly once even though byte prefixes are computed without
/// temporary Strings.
pub(crate) fn write_relation_manifest_v2_observed<
    W: fmt::Write,
    O: ParametricRelationV2Observer,
>(
    writer: &mut W,
    relation: &ParametricRelation,
    observer: &mut O,
) -> fmt::Result {
    writer.write_str(PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA)?;
    writer.write_str("|family=")?;
    write_length_prefixed_manifest_payload(writer, relation.family_fingerprint.as_ref(), observer)?;
    writer.write_str("|context=")?;
    write_length_prefixed_manifest_payload(
        writer,
        relation.context_fingerprint.as_ref(),
        observer,
    )?;
    writer.write_str("|row=")?;
    write_length_prefixed_manifest_payload(writer, &relation.row_id, observer)?;
    writer.write_str("|arity=")?;
    observe_usize(observer, relation.arity)?;
    write!(writer, "{}", relation.arity)?;
    writer.write_str("|terms=")?;
    observe_usize(observer, relation.terms.len())?;
    write!(writer, "{}", relation.terms.len())?;
    for (shift, coefficient) in &relation.terms {
        writer.write_str("|shift=")?;
        write_length_prefixed_manifest_payload(writer, shift, observer)?;
        writer.write_str("|coefficient=")?;
        write_length_prefixed_manifest_payload(writer, coefficient.raw(), observer)?;
    }
    writer.write_str("|guards=")?;
    observe_usize(observer, relation.guarded_nonzero.len())?;
    write!(writer, "{}", relation.guarded_nonzero.len())?;
    for condition in &relation.guarded_nonzero {
        writer.write_str("|polynomial=")?;
        write_length_prefixed_manifest_payload(writer, condition.polynomial().raw(), observer)?;
        writer.write_str("|origins=")?;
        observe_usize(observer, condition.origins().len())?;
        write!(writer, "{}", condition.origins().len())?;
        for origin in condition.origins() {
            writer.write_str("|origin=")?;
            write_length_prefixed_manifest_payload(writer, origin, observer)?;
        }
    }
    Ok(())
}

impl ParametricRelation {
    pub fn new(
        family_fingerprint: impl Into<Arc<str>>,
        row_id: ParametricRowId,
        context: &ParametricCoefficientContext,
    ) -> Self {
        Self {
            family_fingerprint: family_fingerprint.into(),
            context_fingerprint: context.fingerprint().into(),
            row_id,
            arity: context.index_count(),
            terms: BTreeMap::new(),
            nonzero: Vec::new(),
            guarded_nonzero: Vec::new(),
        }
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Exact authenticated `K(n)` identity used by this relation.
    ///
    /// Operator adapters use this to reject even empty relations from a
    /// foreign index scope; inspecting term coefficients alone would not be
    /// sufficient for a zero row.
    pub fn context_fingerprint(&self) -> &str {
        &self.context_fingerprint
    }

    pub fn row_id(&self) -> &ParametricRowId {
        &self.row_id
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn terms(&self) -> &BTreeMap<IndexShift, ParametricCoefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[ParametricPolynomial] {
        &self.nonzero
    }

    pub fn guarded_nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.guarded_nonzero
    }

    /// Conservative owned bytes for retaining or deeply cloning this sparse
    /// row. Shared family/context/row-label `Arc` payloads remain external
    /// seams; all B-tree nodes, shifts, sparse polynomial vectors, GMP spare
    /// limbs, compatibility guards, guarded conditions, and origins are
    /// charged.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let btree_node_bound = size_of::<(IndexShift, ParametricCoefficient)>()
            .checked_mul(16)?
            .checked_add(32usize.checked_mul(size_of::<usize>())?)?;
        let mut bytes = size_of::<Self>();
        for (shift, coefficient) in &self.terms {
            bytes = bytes.checked_add(btree_node_bound)?;
            bytes = bytes.checked_add(shift.0.capacity().checked_mul(size_of::<i64>())?)?;
            bytes = bytes.checked_add(coefficient.owned_retained_byte_bound()?)?;
        }
        bytes = bytes.checked_add(
            self.nonzero
                .capacity()
                .checked_mul(size_of::<ParametricPolynomial>())?,
        )?;
        for polynomial in &self.nonzero {
            bytes = bytes.checked_add(polynomial.owned_retained_byte_bound()?)?;
        }
        bytes = bytes.checked_add(
            self.guarded_nonzero
                .capacity()
                .checked_mul(size_of::<ParametricNonZeroCondition>())?,
        )?;
        for condition in &self.guarded_nonzero {
            bytes = bytes.checked_add(condition.owned_retained_byte_bound()?)?;
        }
        Some(bytes)
    }

    /// Compare both mathematical relation content and complete guard history.
    /// Ordinary `PartialEq` intentionally preserves the legacy mathematical
    /// equality semantics and does not compare adapter-history atoms.
    pub fn has_identical_guard_provenance(&self, other: &Self) -> bool {
        self == other && self.guarded_nonzero == other.guarded_nonzero
    }

    /// Lossless, deterministic identity of the mathematical row and every
    /// retained exceptional-domain origin.
    ///
    /// This intentionally returns the complete length-prefixed manifest, not
    /// a collision-prone process hash. Elimination and persistence layers may
    /// hash this byte string for indexing, but authentication compares the
    /// manifest itself.
    pub fn stable_manifest(&self) -> String {
        // `usize::MAX` cannot be reached by a live `String`; retain the
        // infallible compatibility API while keeping one canonical encoder.
        self.stable_manifest_with_limit(usize::MAX)
            .expect("an allocated relation manifest fits in usize")
    }

    /// Encode the exact [`Self::stable_manifest`] payload without allowing
    /// the retained output buffer to grow past `max_bytes`.
    ///
    /// The bounded and unbounded APIs share this encoder, so a successful
    /// result is byte-for-byte identical.  This is the persistence-facing
    /// entry point: callers can reject an oversized row before adding it to an
    /// aggregate retained manifest.
    pub fn stable_manifest_with_limit(
        &self,
        max_bytes: usize,
    ) -> Result<String, ParametricRelationError> {
        // Count the complete V2 byte stream before retaining any output.  The
        // second pass writes directly into one fallibly reserved String; no
        // coefficient, polynomial, row-id, or origin sub-String is retained.
        let exact_bytes = self.stable_manifest_byte_len_with_limit(max_bytes)?;

        let mut output = RelationManifestBuilder::try_new(exact_bytes)?;
        {
            let mut sink = RelationManifestSink::Output(&mut output);
            let result = write_relation_manifest_v2(&mut sink, self);
            sink.finish_write(result)?;
        }
        if output.output.len() != exact_bytes {
            return Err(ParametricRelationError::ManifestEncodingMismatch {
                expected_bytes: exact_bytes,
                actual_bytes: output.output.len(),
            });
        }
        Ok(output.finish())
    }

    /// Count the canonical manifest without allocating its retained `String`.
    /// Aggregate certificate builders use this to include the exact manifest
    /// payload in their own byte preflight before requesting the output
    /// buffer.
    pub(crate) fn stable_manifest_byte_len_with_limit(
        &self,
        max_bytes: usize,
    ) -> Result<usize, ParametricRelationError> {
        let mut counter = ManifestByteCounter::new(max_bytes);
        let result = {
            let mut sink = RelationManifestSink::Counter(&mut counter);
            let result = write_relation_manifest_v2(&mut sink, self);
            sink.finish_write(result)
        };
        if let Err(error) = result {
            return Err(error);
        }
        relation_check_limit(
            "parametric relation manifest bytes",
            counter.bytes,
            max_bytes,
        )?;
        Ok(counter.bytes)
    }

    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn add_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
    ) -> Result<(), ParametricRelationError> {
        self.add_nonzero_condition_with_limits(
            context,
            condition,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricPolynomial,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let condition = context.nonzero_condition_with_origins_and_limits(
            condition,
            [GuardOrigin::ExplicitRelationCondition],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, condition, limits)
    }

    pub fn add_guarded_nonzero_condition(
        &mut self,
        context: &ParametricCoefficientContext,
        condition: ParametricNonZeroCondition,
    ) -> Result<(), ParametricRelationError> {
        self.add_guarded_nonzero_condition_with_limits(
            context,
            condition,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_guarded_nonzero_condition_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        mut condition: ParametricNonZeroCondition,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        if !context.contains_nonzero_condition(&condition) {
            return Err(ParametricRelationError::WrongContext);
        }
        context.validate_polynomial_with_limits(condition.polynomial(), limits.exact_algebra)?;
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_origin_with_limit(
            GuardOrigin::RelationConditionAttached {
                row: self.row_id.guard_identity(),
            },
            limits.max_guard_origins,
        )?;
        let is_new = !self
            .guarded_nonzero
            .iter()
            .any(|existing| existing.polynomial() == condition.polynomial());
        let polynomial = condition.polynomial().clone();
        insert_parametric_condition(
            &mut self.guarded_nonzero,
            condition,
            limits.max_guard_origins,
        )?;
        if is_new {
            self.nonzero.push(polynomial);
        }
        Ok(())
    }

    pub fn add_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_term_with_limits(
            context,
            shift,
            coefficient,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_term_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_term_in_place(context, shift, coefficient, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply one term insertion to an isolated relation snapshot.
    ///
    /// The public entry point clones before calling this helper because the
    /// input-denominator guard is discovered before coefficient collection.
    /// A later exact-arithmetic failure must not leave that guard committed to
    /// an otherwise unchanged relation.
    fn add_term_in_place(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(&shift)?;
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;

        // Inspect the incoming fraction before testing whether its numerator
        // is zero.  This preserves a deliberately unnormalized `0 / p` as a
        // domain-bearing zero term.
        let denominator =
            context.denominator_condition_with_limits(&coefficient, limits.exact_algebra)?;
        let condition = context.nonzero_condition_with_origins_and_limits(
            denominator,
            [GuardOrigin::RelationInputTermDenominator {
                row: self.row_id.guard_identity(),
                shift: shift.values().to_vec().into_boxed_slice(),
            }],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&shift) {
            let sum = context.add_with_limits(current, &coefficient, limits.exact_algebra)?;
            if sum.is_zero() {
                self.terms.remove(&shift);
            } else {
                let denominator =
                    context.denominator_condition_with_limits(&sum, limits.exact_algebra)?;
                let condition = context.nonzero_condition_with_origins_and_limits(
                    denominator,
                    [GuardOrigin::RelationCollectedTermDenominator {
                        row: self.row_id.guard_identity(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    }],
                    limits.exact_algebra,
                )?;
                self.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
                self.terms.insert(shift, sum);
            }
        } else {
            self.terms.insert(shift, coefficient);
        }
        Ok(())
    }

    /// Insert one already domain-audited coefficient at a distinct shift.
    ///
    /// This is intentionally crate-private and narrower than [`Self::add_term`].
    /// It is used by locus-bound compilers which have separately preserved and
    /// classified the coefficient's pre-normalization denominator.  Calling
    /// the public insertion path again would rediscover a normalized
    /// denominator and could incorrectly move a base-only assumption back
    /// into the private index-guard set.
    ///
    /// The caller must prove that source-key translation is injective.  A
    /// collision is rejected rather than collected because collection could
    /// introduce a new denominator that has not passed that proof boundary.
    pub(crate) fn insert_prevalidated_distinct_term_without_denominator_discovery(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(&shift)?;
        context.validate_with_limits(&coefficient, limits.exact_algebra)?;
        if coefficient.is_zero() {
            return Ok(());
        }
        if self.terms.contains_key(&shift) {
            return Err(ParametricRelationError::PrevalidatedTermCollision);
        }
        self.terms.insert(shift, coefficient);
        Ok(())
    }

    /// Insert a coefficient returned by guarded division.  Its exceptional
    /// domain is retained even when the normalized value is zero.
    pub fn add_guarded_term_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: GuardedParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        for condition in coefficient.nonzero {
            staged.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        }
        staged.add_term_in_place(context, shift, coefficient.value, limits)?;
        *self = staged;
        Ok(())
    }

    pub fn add_guarded_term(
        &mut self,
        context: &ParametricCoefficientContext,
        shift: IndexShift,
        coefficient: GuardedParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_guarded_term_with_limits(
            context,
            shift,
            coefficient,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_scaled(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_scaled_with_limits(
            context,
            other,
            factor,
            ParametricArithmeticLimits::default(),
        )
    }

    pub fn add_scaled_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.add_scaled_in_place(context, other, factor, limits)?;
        *self = staged;
        Ok(())
    }

    /// Apply a scaled addition to an isolated relation snapshot.
    fn add_scaled_in_place(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: &ParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        self.validate_compatible(other, context)?;
        context.validate_with_limits(factor, limits.exact_algebra)?;
        for condition in &other.guarded_nonzero {
            self.add_guarded_nonzero_condition_with_limits(context, condition.clone(), limits)?;
        }
        let factor_denominator =
            context.denominator_condition_with_limits(factor, limits.exact_algebra)?;
        let factor_condition = context.nonzero_condition_with_origins_and_limits(
            factor_denominator,
            [GuardOrigin::RelationScaleFactorDenominator {
                target_row: self.row_id.guard_identity(),
                source_row: other.row_id.guard_identity(),
            }],
            limits.exact_algebra,
        )?;
        self.add_guarded_nonzero_condition_with_limits(context, factor_condition, limits)?;
        for (shift, coefficient) in &other.terms {
            let scaled = context.mul_with_limits(coefficient, factor, limits.exact_algebra)?;
            self.add_term_in_place(context, shift.clone(), scaled, limits)?;
        }
        Ok(())
    }

    /// Add a relation with a scalar returned by guarded division.  This is
    /// the elimination-facing counterpart of [`Self::add_guarded_term`].
    pub fn add_scaled_guarded_with_limits(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: GuardedParametricCoefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        let mut staged = self.clone();
        staged.validate_compatible(other, context)?;
        context.validate_with_limits(&factor.value, limits.exact_algebra)?;
        for condition in factor.nonzero {
            staged.add_guarded_nonzero_condition_with_limits(context, condition, limits)?;
        }
        staged.add_scaled_in_place(context, other, &factor.value, limits)?;
        *self = staged;
        Ok(())
    }

    pub fn add_scaled_guarded(
        &mut self,
        context: &ParametricCoefficientContext,
        other: &Self,
        factor: GuardedParametricCoefficient,
    ) -> Result<(), ParametricRelationError> {
        self.add_scaled_guarded_with_limits(
            context,
            other,
            factor,
            ParametricArithmeticLimits::default(),
        )
    }

    /// Recenter a residual-affine pivot without conflating its two lattice
    /// actions.
    ///
    /// Coefficients and guards undergo `n -> n + coefficient_translation`,
    /// while every integral key undergoes `q -> q - key_center`.  The latter
    /// subtraction is checked componentwise, so an `i64` boundary remains a
    /// typed rejection at the caller rather than wrapping into a fabricated
    /// recurrence.  The complete translation/output envelope is checked
    /// before the first Symbolica substitution is executed.
    pub(crate) fn affine_free_recentered(
        &self,
        context: &ParametricCoefficientContext,
        coefficient_translation: &IndexShift,
        key_center: &IndexShift,
        row_id: ParametricRowId,
        limits: ParametricAffineFreeRecenteringLimits,
    ) -> Result<(Self, ParametricAffineFreeRecenteringStats), ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(coefficient_translation)?;
        self.validate_shift(key_center)?;

        relation_check_limit(
            "affine free recentering terms",
            self.terms.len(),
            limits.max_terms,
        )?;
        relation_check_limit(
            "affine free recentering guards",
            self.guarded_nonzero.len(),
            limits.max_guards,
        )?;
        let translation_components = relation_checked_mul(
            "affine free recentering translation components",
            self.arity,
            2,
        )?;
        relation_check_limit(
            "affine free recentering translation components",
            translation_components,
            limits.max_translation_components,
        )?;

        let key_subtraction_boundary_checks = relation_checked_mul(
            "affine free recentering key-subtraction boundary checks",
            self.terms.len(),
            self.arity,
        )?;
        relation_check_limit(
            "affine free recentering key-subtraction boundary checks",
            key_subtraction_boundary_checks,
            limits.max_key_subtraction_boundary_checks,
        )?;

        // Validate every key subtraction before allocating any target key or
        // entering Symbolica.  Translation by a common center is injective,
        // so the execution phase may use the distinct-key insertion seam.
        for shift in self.terms.keys() {
            for (position, (&value, &center)) in
                shift.values().iter().zip(key_center.values()).enumerate()
            {
                value
                    .checked_sub(center)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?;
            }
        }

        let source_row = self.row_id.guard_identity();
        let target_row = row_id.guard_identity();
        let btree_node_bound = size_of::<(IndexShift, ParametricCoefficient)>()
            .checked_mul(16)
            .and_then(|bytes| bytes.checked_add(32usize.checked_mul(size_of::<usize>())?))
            .ok_or(ParametricRelationError::ResourceCountOverflow {
                resource: "affine free recentering retained bytes",
            })?;
        // Both canonical and compatibility guard vectors grow independently.
        // Reserve their complete source-cardinality up front below; charge a
        // conservative two slots per source guard here before any translated
        // polynomial or provenance atom is constructed.
        let guard_capacity_slots = relation_checked_mul(
            "affine free recentering guard vector capacity slots",
            self.guarded_nonzero.len(),
            2,
        )?;
        let guard_vector_capacity_bytes = relation_checked_mul(
            "affine free recentering retained bytes",
            guard_capacity_slots,
            relation_checked_add(
                "affine free recentering retained bytes",
                size_of::<ParametricPolynomial>(),
                size_of::<ParametricNonZeroCondition>(),
            )?,
        )?;
        let retained_bytes = relation_checked_add(
            "affine free recentering retained bytes",
            size_of::<Self>(),
            guard_vector_capacity_bytes,
        )?;
        relation_check_limit(
            "affine free recentering retained bytes",
            retained_bytes,
            limits.max_retained_bytes,
        )?;
        let mut stats = ParametricAffineFreeRecenteringStats {
            terms: self.terms.len(),
            guards: self.guarded_nonzero.len(),
            translation_components,
            key_subtraction_boundary_checks,
            retained_bytes,
            ..Default::default()
        };

        for coefficient in self.terms.values() {
            let preflight = context.preflight_translate_coefficient(
                coefficient,
                coefficient_translation,
                limits.arithmetic,
            )?;
            stats.source_terms = relation_bounded_add(
                "affine free recentering source terms",
                stats.source_terms,
                preflight.source_terms(),
                limits.max_source_terms,
            )?;
            let source_exponent_entries = relation_checked_add(
                "affine free recentering source exponent entries",
                preflight.numerator().source_exponent_entries(),
                preflight.denominator().source_exponent_entries(),
            )?;
            stats.source_exponent_entries = relation_bounded_add(
                "affine free recentering source exponent entries",
                stats.source_exponent_entries,
                source_exponent_entries,
                limits.max_source_exponent_entries,
            )?;
            stats.output_terms = relation_bounded_add(
                "affine free recentering output terms",
                stats.output_terms,
                preflight.output_term_bound(),
                limits.max_output_terms,
            )?;
            let output_exponent_entries = relation_checked_add(
                "affine free recentering output exponent entries",
                preflight.numerator().output_exponent_entry_bound(),
                preflight.denominator().output_exponent_entry_bound(),
            )?;
            stats.output_exponent_entries = relation_bounded_add(
                "affine free recentering output exponent entries",
                stats.output_exponent_entries,
                output_exponent_entries,
                limits.max_output_exponent_entries,
            )?;
            stats.power_operations = relation_bounded_add(
                "affine free recentering power operations",
                stats.power_operations,
                preflight.power_operation_bound(),
                limits.max_power_operations,
            )?;
            stats.integer_bit_work = relation_bounded_add(
                "affine free recentering integer-bit work",
                stats.integer_bit_work,
                preflight.integer_bit_work_bound(),
                limits.max_integer_bit_work,
            )?;
            stats.normalized_coefficient_terms = relation_bounded_add(
                "affine free recentering normalized coefficient terms",
                stats.normalized_coefficient_terms,
                preflight.normalized_coefficient_term_bound(),
                limits.max_normalized_coefficient_terms,
            )?;
            let term_bytes = relation_checked_add(
                "affine free recentering retained bytes",
                btree_node_bound,
                relation_checked_add(
                    "affine free recentering retained bytes",
                    relation_checked_mul(
                        "affine free recentering retained bytes",
                        self.arity,
                        size_of::<i64>(),
                    )?,
                    preflight.normalized_coefficient_byte_bound(),
                )?,
            )?;
            stats.retained_bytes = relation_bounded_add(
                "affine free recentering retained bytes",
                stats.retained_bytes,
                term_bytes,
                limits.max_retained_bytes,
            )?;
        }

        for condition in &self.guarded_nonzero {
            // Translation may add IndexTranslation; this helper adds its
            // split-operation atom; insertion may add the target-row atom.
            // Check the conservative complete count before any of those
            // provenance allocations are requested.
            let prospective_origins = relation_checked_add(
                "affine free recentering guard origins",
                condition.origins().len(),
                3,
            )?;
            relation_check_limit(
                "affine free recentering guard origins",
                prospective_origins,
                limits.arithmetic.max_guard_origins,
            )?;
            let preflight = context.preflight_translate_polynomial(
                condition.polynomial(),
                coefficient_translation,
                limits.arithmetic,
            )?;
            stats.source_terms = relation_bounded_add(
                "affine free recentering source terms",
                stats.source_terms,
                preflight.source_terms(),
                limits.max_source_terms,
            )?;
            stats.source_exponent_entries = relation_bounded_add(
                "affine free recentering source exponent entries",
                stats.source_exponent_entries,
                preflight.source_exponent_entries(),
                limits.max_source_exponent_entries,
            )?;
            stats.output_terms = relation_bounded_add(
                "affine free recentering output terms",
                stats.output_terms,
                preflight.output_term_bound(),
                limits.max_output_terms,
            )?;
            stats.output_exponent_entries = relation_bounded_add(
                "affine free recentering output exponent entries",
                stats.output_exponent_entries,
                preflight.output_exponent_entry_bound(),
                limits.max_output_exponent_entries,
            )?;
            stats.power_operations = relation_bounded_add(
                "affine free recentering power operations",
                stats.power_operations,
                preflight.power_operation_bound(),
                limits.max_power_operations,
            )?;
            stats.integer_bit_work = relation_bounded_add(
                "affine free recentering integer-bit work",
                stats.integer_bit_work,
                preflight.integer_bit_work_bound(),
                limits.max_integer_bit_work,
            )?;

            let mut guard_bytes = relation_checked_mul(
                "affine free recentering retained bytes",
                preflight.retained_output_byte_bound(),
                2,
            )?;
            guard_bytes = relation_checked_add(
                "affine free recentering retained bytes",
                guard_bytes,
                size_of::<ParametricNonZeroCondition>()
                    .checked_add(size_of::<ParametricPolynomial>())
                    .ok_or(ParametricRelationError::ResourceCountOverflow {
                        resource: "affine free recentering retained bytes",
                    })?,
            )?;
            // Charge every source origin separately. The three prospective
            // atoms below are IndexTranslation, this split recentering, and
            // attachment to the new row. Duplicate atoms only make this
            // envelope more conservative.
            for origin in condition.origins() {
                guard_bytes = relation_checked_add(
                    "affine free recentering retained bytes",
                    guard_bytes,
                    origin.retained_byte_bound().ok_or(
                        ParametricRelationError::ResourceCountOverflow {
                            resource: "affine free recentering retained bytes",
                        },
                    )?,
                )?;
            }
            for origin_bytes in [
                GuardOrigin::index_translation_retained_byte_bound(self.arity),
                GuardOrigin::relation_affine_free_recentering_retained_byte_bound(
                    &source_row,
                    &target_row,
                    self.arity,
                    self.arity,
                ),
                GuardOrigin::relation_attached_retained_byte_bound(&target_row),
            ] {
                guard_bytes = relation_checked_add(
                    "affine free recentering retained bytes",
                    guard_bytes,
                    origin_bytes.ok_or(ParametricRelationError::ResourceCountOverflow {
                        resource: "affine free recentering retained bytes",
                    })?,
                )?;
            }
            stats.retained_bytes = relation_bounded_add(
                "affine free recentering retained bytes",
                stats.retained_bytes,
                guard_bytes,
                limits.max_retained_bytes,
            )?;
        }
        relation_check_limit(
            "affine free recentering retained bytes",
            stats.retained_bytes,
            limits.max_retained_bytes,
        )?;

        let copy_vector = |values: &[i64]| -> Result<Vec<i64>, ParametricRelationError> {
            let mut copy = Vec::new();
            copy.try_reserve_exact(values.len()).map_err(|_| {
                ParametricRelationError::AllocationFailure {
                    resource: "affine free recentering provenance components",
                    requested: values.len(),
                }
            })?;
            copy.extend_from_slice(values);
            Ok(copy)
        };
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        try_reserve_relation_entries(
            "affine free recentering compatibility guards",
            &mut result.nonzero,
            self.guarded_nonzero.len(),
        )?;
        try_reserve_relation_entries(
            "affine free recentering guarded conditions",
            &mut result.guarded_nonzero,
            self.guarded_nonzero.len(),
        )?;
        for capacity in [result.nonzero.capacity(), result.guarded_nonzero.capacity()] {
            relation_check_limit(
                "affine free recentering guard vector capacity slots",
                capacity,
                guard_capacity_slots,
            )?;
        }
        for condition in &self.guarded_nonzero {
            let mut translated = context.translate_nonzero_condition(
                condition,
                coefficient_translation.values(),
                limits.arithmetic,
            )?;
            translated.add_origin_with_limit(
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    coefficient_offset: copy_vector(coefficient_translation.values())?,
                    key_center: copy_vector(key_center.values())?,
                },
                limits.arithmetic.max_guard_origins,
            )?;
            result.add_guarded_nonzero_condition_with_limits(
                context,
                translated,
                limits.arithmetic,
            )?;
        }
        for (shift, coefficient) in &self.terms {
            let mut centered = Vec::new();
            centered.try_reserve_exact(self.arity).map_err(|_| {
                ParametricRelationError::AllocationFailure {
                    resource: "affine free recentering key components",
                    requested: self.arity,
                }
            })?;
            for (position, (&value, &center)) in
                shift.values().iter().zip(key_center.values()).enumerate()
            {
                centered.push(
                    value
                        .checked_sub(center)
                        .ok_or(ParametricRelationError::IndexOverflow { position })?,
                );
            }
            let centered = IndexShift::try_from_preallocated(centered, self.arity)?;
            let translated = context.translate(
                coefficient,
                coefficient_translation.values(),
                limits.arithmetic,
            )?;
            result.insert_prevalidated_distinct_term_without_denominator_discovery(
                context,
                centered,
                translated,
                limits.arithmetic,
            )?;
        }
        let observed_owned_bytes = result.owned_retained_byte_bound().ok_or(
            ParametricRelationError::ResourceCountOverflow {
                resource: "affine free recentering observed retained bytes",
            },
        )?;
        relation_check_limit(
            "affine free recentering retained-byte envelope",
            observed_owned_bytes,
            stats.retained_bytes,
        )?;
        Ok((result, stats))
    }

    /// Translate keys and every index occurrence in coefficients/conditions
    /// as one operation.
    pub fn translated(
        &self,
        context: &ParametricCoefficientContext,
        translation: &IndexShift,
        row_id: ParametricRowId,
        limits: ParametricArithmeticLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        self.validate_shift(translation)?;
        let target_row = row_id.guard_identity();
        let source_row = self.row_id.guard_identity();
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        for condition in &self.guarded_nonzero {
            let mut translated =
                context.translate_nonzero_condition(condition, translation.values(), limits)?;
            translated.add_origin_with_limit(
                GuardOrigin::RelationTranslation {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    offset: translation.values().to_vec().into_boxed_slice(),
                },
                limits.max_guard_origins,
            )?;
            result.add_guarded_nonzero_condition_with_limits(context, translated, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let translated_shift = shift.checked_add(translation)?;
            let translated_coefficient =
                context.translate(coefficient, translation.values(), limits)?;
            // `result` is an isolated, not-yet-published relation.  Use the
            // transactional helper directly so translating many terms does
            // not deep-clone every previously retained guard and origin on
            // each insertion; any error still drops the complete local row.
            result.add_term_in_place(context, translated_shift, translated_coefficient, limits)?;
        }
        Ok(result)
    }

    /// Transport this complete global identity through a simultaneous
    /// permutation of denominator powers and coefficient index variables.
    ///
    /// `source_to_target[i] = j` acts on both halves of every parametric term:
    /// `s_source[i] -> s_target[j]` and `n_source[i] -> n_target[j]`.  Applying
    /// only the shift half would be unsound because a verified integral
    /// symmetry maps `I(n+s)` to `I(Pn+Ps)`, not to `I(n+Ps)` for generic
    /// independent indices.
    pub fn permuted_indices(
        &self,
        context: &ParametricCoefficientContext,
        source_to_target: &[usize],
        row_id: ParametricRowId,
        limits: ParametricArithmeticLimits,
    ) -> Result<Self, ParametricRelationError> {
        self.validate_context(context)?;
        if source_to_target.len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: source_to_target.len(),
            });
        }
        // Authenticate the permutation through the same coefficient API used
        // below before allocating a transported relation.
        context.permute_indices(&context.one(), source_to_target, limits)?;
        let target_row = row_id.guard_identity();
        let source_row = self.row_id.guard_identity();
        let mut result = Self::new(self.family_fingerprint.clone(), row_id, context);
        for condition in &self.guarded_nonzero {
            let mut permuted =
                context.permute_nonzero_condition_indices(condition, source_to_target, limits)?;
            permuted.add_origin_with_limit(
                GuardOrigin::RelationIndexPermutation {
                    source_row: source_row.clone(),
                    target_row: target_row.clone(),
                    source_to_target: source_to_target.to_vec().into_boxed_slice(),
                },
                limits.max_guard_origins,
            )?;
            result.add_guarded_nonzero_condition_with_limits(context, permuted, limits)?;
        }
        for (shift, coefficient) in &self.terms {
            let mut target_shift = vec![0_i64; self.arity];
            for (source_index, &target_index) in source_to_target.iter().enumerate() {
                target_shift[target_index] = shift.values()[source_index];
            }
            let target_shift = IndexShift::try_new(target_shift, self.arity)?;
            let target_coefficient =
                context.permute_indices(coefficient, source_to_target, limits)?;
            result.add_term_with_limits(context, target_shift, target_coefficient, limits)?;
        }
        Ok(result)
    }

    pub fn specialize(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, ParametricRelationError> {
        self.specialize_with_additional_nonzero_conditions(
            context,
            assignment,
            std::iter::empty::<&ParametricNonZeroCondition>(),
            limits,
        )
    }

    /// Prepare one complete concrete specialization without allocating any
    /// output polynomial, guard, key, or relation container.
    ///
    /// Additional conditions are a borrowed slice rather than a one-shot
    /// iterator: preflight must inspect the complete stream before execution
    /// is authorized, and execution must then replay precisely that stream.
    pub(crate) fn prepare_concrete_specialization_with_additional_nonzero_conditions<'a>(
        &'a self,
        context: &'a ParametricCoefficientContext,
        assignment: &'a [i64],
        additional_nonzero_conditions: &'a [ParametricNonZeroCondition],
        limits: ParametricConcreteSpecializationLimits,
    ) -> Result<ParametricConcreteSpecializationPlan<'a>, ParametricRelationError> {
        self.validate_context(context)?;
        if assignment.len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: assignment.len(),
            });
        }
        let preflight = preflight_concrete_specialization(
            self,
            context,
            assignment,
            additional_nonzero_conditions,
            limits,
        )?;
        Ok(ParametricConcreteSpecializationPlan {
            relation: self,
            context,
            assignment,
            additional_nonzero_conditions,
            limits,
            preflight,
        })
    }

    /// Specialize while conjoining proof-owned conditions without cloning
    /// either this relation or those conditions into a temporary relation.
    ///
    /// The relation's own guards retain their existing order. Additional
    /// conditions are then specialized in iterator order before any term, so
    /// an unsatisfied borrowed domain is reported before coefficient or key
    /// specialization begins.
    pub(crate) fn specialize_with_additional_nonzero_conditions<'a>(
        &self,
        context: &ParametricCoefficientContext,
        assignment: &[i64],
        additional_nonzero_conditions: impl IntoIterator<Item = &'a ParametricNonZeroCondition>,
        limits: ParametricArithmeticLimits,
    ) -> Result<ConcreteRelation, ParametricRelationError> {
        self.validate_context(context)?;
        if assignment.len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: assignment.len(),
            });
        }
        let mut result = ConcreteRelation {
            family_fingerprint: self.family_fingerprint.clone(),
            row_id: self.row_id.clone(),
            arity: self.arity,
            terms: BTreeMap::new(),
            nonzero: Vec::new(),
            guarded_nonzero: Vec::new(),
        };
        for condition in &self.guarded_nonzero {
            specialize_borrowed_nonzero_condition(
                &mut result,
                context,
                condition,
                assignment,
                None,
                limits,
            )?;
        }
        for condition in additional_nonzero_conditions {
            specialize_borrowed_nonzero_condition(
                &mut result,
                context,
                condition,
                assignment,
                Some(GuardOrigin::RelationConditionAttached {
                    row: self.row_id.guard_identity(),
                }),
                limits,
            )?;
        }
        for (shift, coefficient) in &self.terms {
            let specialized = context.specialize(coefficient, assignment, limits)?;
            for condition in specialized.guarded_nonzero_conditions() {
                result.add_guarded_nonzero(context.base(), condition.clone(), limits)?;
            }
            // A symbolic term may vanish at this assignment.  In that case
            // its integral key is semantically absent, so an otherwise
            // overflowing `assignment + shift` must not reject the complete
            // specialization.
            if specialized.value.is_zero() {
                continue;
            }
            let key = ConcreteIntegralKey::checked_from_assignment(assignment, shift)?;
            result.add_term(context.base(), key, specialized.value, limits)?;
        }
        Ok(result)
    }

    /// Rebuild this source identity on a canonical sparse integer equality
    /// locus while leaving every other index symbolic.
    ///
    /// The returned object binds the rebuilt row to its assignment; it does
    /// not expose the row as a globally valid identity.
    /// Specialize while sharing the exact authenticated source allocation.
    /// Replays and cloned certificates retain this same Arc.
    pub fn partially_specialized_on(
        self: &Arc<Self>,
        context: &ParametricCoefficientContext,
        assignment: PartialIndexAssignment,
        limits: PartialParametricRelationSpecializationLimits,
    ) -> Result<PartialParametricRelationSpecialization, ParametricRelationError> {
        self.validate_context(context)?;
        if assignment.arity() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: assignment.arity(),
            });
        }
        relation_check_limit(
            "partial relation assignments",
            assignment.entries().len(),
            limits.max_assignments,
        )?;
        relation_check_limit("partial relation terms", self.terms.len(), limits.max_terms)?;
        relation_check_limit(
            "partial relation guards",
            self.guarded_nonzero.len(),
            limits.max_guards,
        )?;

        let mut relation = Self::new(
            self.family_fingerprint.clone(),
            self.row_id.clone(),
            context,
        );
        let mut base_assumptions = Vec::new();
        let mut stats = PartialParametricRelationSpecializationStats {
            terms: self.terms.len(),
            guards: self.guarded_nonzero.len(),
            ..Default::default()
        };

        for condition in &self.guarded_nonzero {
            let (polynomial, work) = context.partially_specialize_polynomial_checked(
                condition.polynomial(),
                &assignment,
                limits.arithmetic,
            )?;
            accumulate_partial_work(&mut stats, work, limits)?;
            if polynomial.is_zero() {
                return Err(ParametricRelationError::UnsatisfiableDomain);
            }
            let mut specialized = context.nonzero_condition_with_origins_and_origin_limit(
                polynomial,
                condition
                    .origins()
                    .iter()
                    .cloned()
                    .chain([assignment.provenance_origin()]),
                limits.arithmetic.exact_algebra,
                limits.arithmetic.max_guard_origins,
            )?;
            if specialized.polynomial().is_nonzero_constant() {
                continue;
            }
            if context.polynomial_depends_on_indices_with_limits(
                specialized.polynomial(),
                limits.arithmetic.exact_algebra,
            )? {
                relation.add_guarded_nonzero_condition_with_limits(
                    context,
                    specialized,
                    limits.arithmetic,
                )?;
            } else {
                // Attach the target row exactly as an ordinary relation guard
                // would, but retain it separately as a formal K-assumption.
                specialized.add_origin_with_limit(
                    GuardOrigin::RelationConditionAttached {
                        row: self.row_id.guard_identity(),
                    },
                    limits.arithmetic.max_guard_origins,
                )?;
                insert_partial_base_assumption(
                    &mut base_assumptions,
                    specialized,
                    limits.arithmetic.max_guard_origins,
                    limits.max_base_assumptions,
                )?;
            }
        }

        for (shift, coefficient) in &self.terms {
            let specialized = context.partially_specialize_coefficient(
                coefficient,
                &assignment,
                limits.arithmetic,
            )?;
            accumulate_partial_work(&mut stats, specialized.stats(), limits)?;
            for condition in specialized.guarded_nonzero_conditions() {
                let mut condition = condition.clone();
                condition.add_origin_with_limit(
                    GuardOrigin::RelationPartialSpecializationTermDenominator {
                        row: self.row_id.guard_identity(),
                        shift: shift.values().to_vec().into_boxed_slice(),
                    },
                    limits.arithmetic.max_guard_origins,
                )?;
                if condition.polynomial().is_zero() {
                    return Err(ParametricRelationError::UnsatisfiableDomain);
                }
                if condition.polynomial().is_nonzero_constant() {
                    continue;
                }
                if context.polynomial_depends_on_indices_with_limits(
                    condition.polynomial(),
                    limits.arithmetic.exact_algebra,
                )? {
                    relation.add_guarded_nonzero_condition_with_limits(
                        context,
                        condition,
                        limits.arithmetic,
                    )?;
                } else {
                    condition.add_origin_with_limit(
                        GuardOrigin::RelationConditionAttached {
                            row: self.row_id.guard_identity(),
                        },
                        limits.arithmetic.max_guard_origins,
                    )?;
                    insert_partial_base_assumption(
                        &mut base_assumptions,
                        condition,
                        limits.arithmetic.max_guard_origins,
                        limits.max_base_assumptions,
                    )?;
                }
            }
            relation.add_term_with_limits(
                context,
                shift.clone(),
                specialized.value,
                limits.arithmetic,
            )?;
        }

        // Relation insertion may discover normalized coefficient
        // denominators.  Include the complete retained result, not only the
        // source census, in the final aggregate proof budget.
        stats.retained_terms = relation_retained_term_count(self, &relation, &base_assumptions)?;
        relation_check_limit(
            "partial relation retained terms",
            stats.retained_terms,
            limits.max_retained_terms,
        )?;
        stats.retained_bytes = relation_retained_bytes(
            self,
            &relation,
            &base_assumptions,
            &assignment,
            limits.max_retained_bytes,
        )?;
        stats.base_assumptions = base_assumptions.len();

        Ok(PartialParametricRelationSpecialization {
            schema: PARTIAL_PARAMETRIC_RELATION_SPECIALIZATION_V1_SCHEMA,
            source: self.clone(),
            assignment,
            relation,
            base_assumptions: Arc::new(base_assumptions),
            limits,
            stats,
        })
    }

    /// Move an owned source relation into its specialization certificate.
    pub fn partially_specialized_on_owned(
        self,
        context: &ParametricCoefficientContext,
        assignment: PartialIndexAssignment,
        limits: PartialParametricRelationSpecializationLimits,
    ) -> Result<PartialParametricRelationSpecialization, ParametricRelationError> {
        Arc::new(self).partially_specialized_on(context, assignment, limits)
    }

    fn validate_context(
        &self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        if self.context_fingerprint.as_ref() == context.fingerprint()
            && self.arity == context.index_count()
        {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongContext)
        }
    }

    fn validate_shift(&self, shift: &IndexShift) -> Result<(), ParametricRelationError> {
        if shift.arity() == self.arity {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: shift.arity(),
            })
        }
    }

    fn validate_compatible(
        &self,
        other: &Self,
        context: &ParametricCoefficientContext,
    ) -> Result<(), ParametricRelationError> {
        self.validate_context(context)?;
        other.validate_context(context)?;
        if self.family_fingerprint == other.family_fingerprint {
            Ok(())
        } else {
            Err(ParametricRelationError::WrongFamily)
        }
    }
}

fn specialize_borrowed_nonzero_condition(
    result: &mut ConcreteRelation,
    context: &ParametricCoefficientContext,
    condition: &ParametricNonZeroCondition,
    assignment: &[i64],
    relation_attachment: Option<GuardOrigin>,
    limits: ParametricArithmeticLimits,
) -> Result<(), ParametricRelationError> {
    let mut specialized = context.specialize_nonzero_condition(condition, assignment, limits)?;
    if specialized.polynomial().is_zero() {
        return Err(ParametricRelationError::UnsatisfiableDomain);
    }
    if let Some(origin) = relation_attachment {
        specialized.add_origin_with_limit(origin, limits.max_guard_origins)?;
    }
    result.add_guarded_nonzero(context.base(), specialized, limits)
}

fn preflight_concrete_specialization(
    relation: &ParametricRelation,
    context: &ParametricCoefficientContext,
    assignment: &[i64],
    additional_nonzero_conditions: &[ParametricNonZeroCondition],
    limits: ParametricConcreteSpecializationLimits,
) -> Result<ParametricConcreteSpecializationPreflight, ParametricRelationError> {
    let mut stats = ParametricConcreteSpecializationPreflight::default();
    let mut concrete_bytes = size_of::<ConcreteRelation>();
    let mut guard_payload_bytes = 0usize;
    let mut largest_visible_temporary_bytes = 0usize;
    let row = relation.row_id.guard_identity();
    let index_origin_bytes = GuardOrigin::index_specialization_retained_byte_bound(relation.arity)
        .ok_or(ParametricRelationError::ResourceCountOverflow {
            resource: "concrete specialization guard-origin retained bytes",
        })?;
    let attached_origin_bytes = GuardOrigin::relation_attached_retained_byte_bound(&row).ok_or(
        ParametricRelationError::ResourceCountOverflow {
            resource: "concrete specialization guard-origin retained bytes",
        },
    )?;
    let denominator_origin_bytes = GuardOrigin::CoefficientSpecializationDenominator
        .retained_byte_bound()
        .ok_or(ParametricRelationError::ResourceCountOverflow {
            resource: "concrete specialization guard-origin retained bytes",
        })?;

    for condition in relation
        .guarded_nonzero
        .iter()
        .chain(additional_nonzero_conditions)
    {
        if !context.contains_nonzero_condition(condition) {
            return Err(ParametricCoefficientError::WrongContext.into());
        }
        let polynomial = context.preflight_specialize_polynomial(
            condition.polynomial(),
            assignment,
            limits.arithmetic,
        )?;
        charge_concrete_polynomial_preflight(&mut stats, polynomial, limits)?;

        stats.guard_occurrence_bound = concrete_specialization_bounded_add(
            "concrete specialization guard occurrences",
            stats.guard_occurrence_bound,
            1,
            limits.max_guard_occurrence_bound,
        )?;
        let occurrence_origins = condition.origins().len().checked_add(2).ok_or(
            ParametricRelationError::ResourceCountOverflow {
                resource: "concrete specialization guard-origin occurrences",
            },
        )?;
        stats.guard_origin_occurrence_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-origin occurrences",
            stats.guard_origin_occurrence_bound,
            occurrence_origins,
            limits.max_guard_origin_occurrence_bound,
        )?;
        let source_origin_bytes = condition.origins().iter().try_fold(
            0usize,
            |total, origin| -> Result<usize, ParametricRelationError> {
                let bytes = origin.retained_byte_bound().ok_or(
                    ParametricRelationError::ResourceCountOverflow {
                        resource: "concrete specialization guard-origin retained bytes",
                    },
                )?;
                relation_checked_add(
                    "concrete specialization guard-origin retained bytes",
                    total,
                    bytes,
                )
            },
        )?;
        let occurrence_origin_bytes = relation_checked_add(
            "concrete specialization guard-origin retained bytes",
            source_origin_bytes,
            relation_checked_add(
                "concrete specialization guard-origin retained bytes",
                index_origin_bytes,
                attached_origin_bytes,
            )?,
        )?;
        stats.guard_origin_retained_byte_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-origin retained bytes",
            stats.guard_origin_retained_byte_bound,
            occurrence_origin_bytes,
            limits.max_guard_origin_retained_byte_bound,
        )?;
        let doubled_polynomial_bytes = relation_checked_mul(
            "concrete specialization guard-polynomial retained bytes",
            polynomial.retained_output_byte_bound(),
            2,
        )?;
        stats.guard_polynomial_retained_byte_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-polynomial retained bytes",
            stats.guard_polynomial_retained_byte_bound,
            doubled_polynomial_bytes,
            limits.max_guard_polynomial_retained_byte_bound,
        )?;
        guard_payload_bytes = relation_checked_add(
            "concrete specialization prospective relation bytes",
            guard_payload_bytes,
            relation_checked_add(
                "concrete specialization prospective relation bytes",
                doubled_polynomial_bytes,
                relation_checked_add(
                    "concrete specialization prospective relation bytes",
                    size_of::<SpecializedNonZeroCondition>(),
                    occurrence_origin_bytes,
                )?,
            )?,
        )?;
        let temporary = relation_checked_add(
            "concrete specialization visible temporary bytes",
            polynomial.retained_output_byte_bound(),
            relation_checked_add(
                "concrete specialization visible temporary bytes",
                size_of::<SpecializedNonZeroCondition>(),
                occurrence_origin_bytes,
            )?,
        )?;
        largest_visible_temporary_bytes = largest_visible_temporary_bytes.max(temporary);
    }

    let term_node_bytes = concrete_relation_term_node_byte_bound()?;
    let key_bytes = concrete_key_retained_byte_bound(relation.arity)?;
    for coefficient in relation.terms.values() {
        let coefficient =
            context.preflight_specialize_coefficient(coefficient, assignment, limits.arithmetic)?;
        charge_concrete_coefficient_preflight(&mut stats, coefficient, limits)?;
        stats.key_component_bound = concrete_specialization_bounded_add(
            "concrete specialization key components",
            stats.key_component_bound,
            relation.arity,
            limits.max_key_component_bound,
        )?;

        concrete_bytes = relation_checked_add(
            "concrete specialization prospective relation bytes",
            concrete_bytes,
            relation_checked_add(
                "concrete specialization prospective relation bytes",
                term_node_bytes,
                relation_checked_add(
                    "concrete specialization prospective relation bytes",
                    key_bytes,
                    coefficient.normalized_coefficient_byte_bound(),
                )?,
            )?,
        )?;

        // Every source denominator may become a nonconstant concrete guard.
        // Treat all of them as distinct occurrences; specialization and final
        // insertion can only discharge or merge this prospective payload.
        stats.guard_occurrence_bound = concrete_specialization_bounded_add(
            "concrete specialization guard occurrences",
            stats.guard_occurrence_bound,
            1,
            limits.max_guard_occurrence_bound,
        )?;
        stats.guard_origin_occurrence_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-origin occurrences",
            stats.guard_origin_occurrence_bound,
            3,
            limits.max_guard_origin_occurrence_bound,
        )?;
        let occurrence_origin_bytes = relation_checked_add(
            "concrete specialization guard-origin retained bytes",
            denominator_origin_bytes,
            relation_checked_add(
                "concrete specialization guard-origin retained bytes",
                index_origin_bytes,
                attached_origin_bytes,
            )?,
        )?;
        stats.guard_origin_retained_byte_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-origin retained bytes",
            stats.guard_origin_retained_byte_bound,
            occurrence_origin_bytes,
            limits.max_guard_origin_retained_byte_bound,
        )?;
        let doubled_polynomial_bytes = relation_checked_mul(
            "concrete specialization guard-polynomial retained bytes",
            coefficient.denominator_guard_byte_bound(),
            2,
        )?;
        stats.guard_polynomial_retained_byte_bound = concrete_specialization_bounded_add(
            "concrete specialization guard-polynomial retained bytes",
            stats.guard_polynomial_retained_byte_bound,
            doubled_polynomial_bytes,
            limits.max_guard_polynomial_retained_byte_bound,
        )?;
        guard_payload_bytes = relation_checked_add(
            "concrete specialization prospective relation bytes",
            guard_payload_bytes,
            relation_checked_add(
                "concrete specialization prospective relation bytes",
                doubled_polynomial_bytes,
                relation_checked_add(
                    "concrete specialization prospective relation bytes",
                    size_of::<SpecializedNonZeroCondition>(),
                    occurrence_origin_bytes,
                )?,
            )?,
        )?;

        // `ParametricCoefficientContext::specialize` can temporarily own the
        // mapped numerator and denominator, normalized coefficient, and two
        // guard-polynomial copies. Its two one-element Vecs are charged with
        // the same conservative growth envelope used for the final relation.
        let one_guard_capacity = concrete_vec_capacity_bound(1)?;
        let temporary_guard_vector_bytes = relation_checked_add(
            "concrete specialization visible temporary bytes",
            relation_checked_mul(
                "concrete specialization visible temporary bytes",
                one_guard_capacity,
                size_of::<BasePolynomial>(),
            )?,
            relation_checked_mul(
                "concrete specialization visible temporary bytes",
                one_guard_capacity,
                size_of::<SpecializedNonZeroCondition>(),
            )?,
        )?;
        let temporary = relation_checked_sum(
            "concrete specialization visible temporary bytes",
            [
                coefficient.numerator().retained_output_byte_bound(),
                coefficient.denominator().retained_output_byte_bound(),
                coefficient.normalized_coefficient_byte_bound(),
                doubled_polynomial_bytes,
                relation_checked_mul(
                    "concrete specialization visible temporary bytes",
                    occurrence_origin_bytes,
                    2,
                )?,
                temporary_guard_vector_bytes,
            ],
        )?;
        largest_visible_temporary_bytes = largest_visible_temporary_bytes.max(temporary);
    }

    // All concrete guards could specialize to the same base polynomial.  A
    // single merged origin tree would then retain the complete occurrence
    // stream, so the per-condition arithmetic cap must admit that worst case
    // before any specialization allocation begins.
    relation_check_limit(
        "concrete specialization merged guard-origin occurrences",
        stats.guard_origin_occurrence_bound,
        limits.arithmetic.max_guard_origins,
    )?;

    let guard_capacity = concrete_vec_capacity_bound(stats.guard_occurrence_bound)?;
    concrete_bytes = relation_checked_sum(
        "concrete specialization prospective relation bytes",
        [
            concrete_bytes,
            relation_checked_mul(
                "concrete specialization prospective relation bytes",
                guard_capacity,
                size_of::<BasePolynomial>(),
            )?,
            relation_checked_mul(
                "concrete specialization prospective relation bytes",
                guard_capacity,
                size_of::<SpecializedNonZeroCondition>(),
            )?,
            guard_payload_bytes,
        ],
    )?;
    relation_check_limit(
        "concrete specialization prospective relation bytes",
        concrete_bytes,
        limits.max_concrete_relation_retained_byte_bound,
    )?;
    stats.concrete_relation_retained_byte_bound = concrete_bytes;
    stats.peak_execution_retained_byte_bound = relation_checked_add(
        "concrete specialization peak visible retained bytes",
        concrete_bytes,
        largest_visible_temporary_bytes,
    )?;
    relation_check_limit(
        "concrete specialization peak visible retained bytes",
        stats.peak_execution_retained_byte_bound,
        limits.max_peak_execution_retained_byte_bound,
    )?;
    Ok(stats)
}

fn charge_concrete_polynomial_preflight(
    stats: &mut ParametricConcreteSpecializationPreflight,
    polynomial: ParametricPolynomialSpecializationPreflight,
    limits: ParametricConcreteSpecializationLimits,
) -> Result<(), ParametricRelationError> {
    stats.source_terms = concrete_specialization_bounded_add(
        "concrete specialization source terms",
        stats.source_terms,
        polynomial.source_terms(),
        limits.max_source_terms,
    )?;
    stats.source_exponent_entries = concrete_specialization_bounded_add(
        "concrete specialization source exponent entries",
        stats.source_exponent_entries,
        polynomial.source_exponent_entries(),
        limits.max_source_exponent_entries,
    )?;
    stats.output_term_bound = concrete_specialization_bounded_add(
        "concrete specialization output terms",
        stats.output_term_bound,
        polynomial.output_term_bound(),
        limits.max_output_term_bound,
    )?;
    stats.output_exponent_entry_bound = concrete_specialization_bounded_add(
        "concrete specialization output exponent entries",
        stats.output_exponent_entry_bound,
        polynomial.output_exponent_entry_bound(),
        limits.max_output_exponent_entry_bound,
    )?;
    stats.power_operation_bound = concrete_specialization_bounded_add(
        "concrete specialization power operations",
        stats.power_operation_bound,
        polynomial.power_operation_bound(),
        limits.max_power_operation_bound,
    )?;
    stats.integer_bit_work_bound = concrete_specialization_bounded_add(
        "concrete specialization integer-bit work",
        stats.integer_bit_work_bound,
        polynomial.integer_bit_work_bound(),
        limits.max_integer_bit_work_bound,
    )?;
    Ok(())
}

fn charge_concrete_coefficient_preflight(
    stats: &mut ParametricConcreteSpecializationPreflight,
    coefficient: ParametricCoefficientSpecializationPreflight,
    limits: ParametricConcreteSpecializationLimits,
) -> Result<(), ParametricRelationError> {
    for polynomial in [coefficient.numerator(), coefficient.denominator()] {
        charge_concrete_polynomial_preflight(stats, polynomial, limits)?;
    }
    stats.normalization_input_term_pair_bound = concrete_specialization_bounded_add(
        "concrete specialization normalization input term pairs",
        stats.normalization_input_term_pair_bound,
        coefficient.normalization_input_term_pair_bound(),
        limits.max_normalization_input_term_pair_bound,
    )?;
    stats.normalized_coefficient_term_bound = concrete_specialization_bounded_add(
        "concrete specialization normalized coefficient terms",
        stats.normalized_coefficient_term_bound,
        coefficient.normalized_coefficient_term_bound(),
        limits.max_normalized_coefficient_term_bound,
    )?;
    stats.normalized_coefficient_retained_byte_bound = concrete_specialization_bounded_add(
        "concrete specialization normalized coefficient retained bytes",
        stats.normalized_coefficient_retained_byte_bound,
        coefficient.normalized_coefficient_byte_bound(),
        limits.max_normalized_coefficient_retained_byte_bound,
    )?;
    Ok(())
}

fn concrete_specialization_bounded_add(
    resource: &'static str,
    current: usize,
    addition: usize,
    limit: usize,
) -> Result<usize, ParametricRelationError> {
    let requested = relation_checked_add(resource, current, addition)?;
    relation_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn relation_checked_sum<const N: usize>(
    resource: &'static str,
    values: [usize; N],
) -> Result<usize, ParametricRelationError> {
    values.into_iter().try_fold(0usize, |total, value| {
        relation_checked_add(resource, total, value)
    })
}

fn concrete_vec_capacity_bound(entries: usize) -> Result<usize, ParametricRelationError> {
    if entries == 0 {
        Ok(0)
    } else {
        Ok(relation_checked_mul("concrete specialization Vec capacity", entries, 2)?.max(8))
    }
}

fn concrete_key_retained_byte_bound(arity: usize) -> Result<usize, ParametricRelationError> {
    relation_checked_mul(
        "concrete specialization key retained bytes",
        concrete_vec_capacity_bound(arity)?,
        size_of::<i64>(),
    )
}

fn concrete_relation_term_node_byte_bound() -> Result<usize, ParametricRelationError> {
    relation_checked_add(
        "concrete specialization relation term node bytes",
        relation_checked_mul(
            "concrete specialization relation term node bytes",
            size_of::<(ConcreteIntegralKey, Coefficient)>(),
            16,
        )?,
        relation_checked_mul(
            "concrete specialization relation term node bytes",
            size_of::<usize>(),
            32,
        )?,
    )
}

fn accumulate_partial_work(
    aggregate: &mut PartialParametricRelationSpecializationStats,
    work: PartialPolynomialSpecializationStats,
    limits: PartialParametricRelationSpecializationLimits,
) -> Result<(), ParametricRelationError> {
    aggregate.source_terms = relation_checked_add(
        "partial relation source terms",
        aggregate.source_terms,
        work.source_terms,
    )?;
    relation_check_limit(
        "partial relation source terms",
        aggregate.source_terms,
        limits.max_source_terms,
    )?;
    aggregate.output_terms = relation_checked_add(
        "partial relation output terms",
        aggregate.output_terms,
        work.output_terms,
    )?;
    relation_check_limit(
        "partial relation output terms",
        aggregate.output_terms,
        limits.max_output_terms,
    )?;
    aggregate.power_operations = relation_checked_add(
        "partial relation power operations",
        aggregate.power_operations,
        work.power_operations,
    )?;
    relation_check_limit(
        "partial relation power operations",
        aggregate.power_operations,
        limits.max_power_operations,
    )?;
    aggregate.integer_bit_work = relation_checked_add(
        "partial relation integer bit work",
        aggregate.integer_bit_work,
        work.integer_bit_bound,
    )?;
    relation_check_limit(
        "partial relation integer bit work",
        aggregate.integer_bit_work,
        limits.max_integer_bit_work,
    )
}

fn insert_partial_base_assumption(
    assumptions: &mut Vec<PartialSpecializationBaseAssumption>,
    condition: ParametricNonZeroCondition,
    max_guard_origins: usize,
    max_base_assumptions: usize,
) -> Result<(), ParametricRelationError> {
    if let Some(existing) = assumptions
        .iter_mut()
        .find(|existing| existing.condition.polynomial() == condition.polynomial())
    {
        existing
            .condition
            .merge_origins_from(&condition, max_guard_origins)?;
    } else {
        let requested =
            relation_checked_add("partial relation base assumptions", assumptions.len(), 1)?;
        relation_check_limit(
            "partial relation base assumptions",
            requested,
            max_base_assumptions,
        )?;
        try_reserve_relation_entries("partial relation base assumptions", assumptions, 1)?;
        assumptions.push(PartialSpecializationBaseAssumption { condition });
    }
    Ok(())
}

fn relation_retained_term_count(
    source: &ParametricRelation,
    relation: &ParametricRelation,
    base_assumptions: &[PartialSpecializationBaseAssumption],
) -> Result<usize, ParametricRelationError> {
    let mut terms = 0usize;
    for retained in [source, relation] {
        for coefficient in retained.terms.values() {
            terms = relation_checked_add(
                "partial relation retained terms",
                terms,
                coefficient.raw().numerator.nterms(),
            )?;
            terms = relation_checked_add(
                "partial relation retained terms",
                terms,
                coefficient.raw().denominator.nterms(),
            )?;
        }
        for condition in &retained.guarded_nonzero {
            terms = relation_checked_add(
                "partial relation retained terms",
                terms,
                condition.polynomial().term_count(),
            )?;
        }
    }
    for assumption in base_assumptions {
        terms = relation_checked_add(
            "partial relation retained terms",
            terms,
            assumption.condition.polynomial().term_count(),
        )?;
    }
    Ok(terms)
}

fn relation_retained_bytes(
    source: &ParametricRelation,
    relation: &ParametricRelation,
    base_assumptions: &[PartialSpecializationBaseAssumption],
    assignment: &PartialIndexAssignment,
    limit: usize,
) -> Result<usize, ParametricRelationError> {
    let mut writer = RelationBoundedByteCounter {
        bytes: 0,
        limit,
        failure: None,
    };
    let write_result = (|| -> fmt::Result {
        write_relation_retained_payload(&mut writer, source)?;
        write_relation_retained_payload(&mut writer, relation)?;
        for &(position, value) in assignment.entries() {
            write!(&mut writer, "{position}={value};")?;
        }
        for assumption in base_assumptions {
            write_typed_polynomial(&mut writer, assumption.condition.polynomial().raw())?;
            writer.write_char(';')?;
            for origin in assumption.condition.origins() {
                origin.write_stable(&mut writer)?;
                writer.write_char(';')?;
            }
        }
        Ok(())
    })();
    if write_result.is_err() {
        return Err(writer.into_error());
    }
    Ok(writer.bytes)
}

fn write_relation_retained_payload(
    writer: &mut RelationBoundedByteCounter,
    relation: &ParametricRelation,
) -> fmt::Result {
    write!(writer, "{}|", relation.family_fingerprint)?;
    write!(writer, "{}|", relation.context_fingerprint)?;
    relation.row_id.write_stable(writer)?;
    writer.write_char('|')?;
    for (shift, coefficient) in &relation.terms {
        for (ordinal, value) in shift.values().iter().enumerate() {
            if ordinal != 0 {
                writer.write_char(',')?;
            }
            write!(writer, "{value}")?;
        }
        writer.write_char(':')?;
        write_typed_coefficient(writer, coefficient.raw())?;
        writer.write_char(';')?;
    }
    for condition in &relation.guarded_nonzero {
        write_typed_polynomial(writer, condition.polynomial().raw())?;
        writer.write_char(';')?;
        for origin in condition.origins() {
            origin.write_stable(writer)?;
            writer.write_char(';')?;
        }
    }
    Ok(())
}

enum RelationBoundedByteCounterFailure {
    Overflow,
    Limit { requested: usize },
}

struct RelationBoundedByteCounter {
    bytes: usize,
    limit: usize,
    failure: Option<RelationBoundedByteCounterFailure>,
}

impl RelationBoundedByteCounter {
    fn into_error(self) -> ParametricRelationError {
        match self.failure {
            Some(RelationBoundedByteCounterFailure::Overflow) => {
                ParametricRelationError::ResourceCountOverflow {
                    resource: "partial relation retained bytes",
                }
            }
            Some(RelationBoundedByteCounterFailure::Limit { requested }) => {
                ParametricRelationError::ResourceLimit {
                    resource: "partial relation retained bytes",
                    requested,
                    limit: self.limit,
                }
            }
            None => ParametricRelationError::ResourceCountOverflow {
                resource: "partial relation retained bytes",
            },
        }
    }
}

impl fmt::Write for RelationBoundedByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(requested) = self.bytes.checked_add(value.len()) else {
            self.failure = Some(RelationBoundedByteCounterFailure::Overflow);
            return Err(fmt::Error);
        };
        if requested > self.limit {
            self.failure = Some(RelationBoundedByteCounterFailure::Limit { requested });
            return Err(fmt::Error);
        }
        self.bytes = requested;
        Ok(())
    }
}

fn relation_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRelationError> {
    left.checked_add(right)
        .ok_or(ParametricRelationError::ResourceCountOverflow { resource })
}

fn relation_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricRelationError> {
    left.checked_mul(right)
        .ok_or(ParametricRelationError::ResourceCountOverflow { resource })
}

fn relation_bounded_add(
    resource: &'static str,
    left: usize,
    right: usize,
    limit: usize,
) -> Result<usize, ParametricRelationError> {
    let requested = relation_checked_add(resource, left, right)?;
    relation_check_limit(resource, requested, limit)?;
    Ok(requested)
}

fn try_reserve_relation_entries<T>(
    resource: &'static str,
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), ParametricRelationError> {
    let requested = relation_checked_add(resource, values.len(), additional)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| ParametricRelationError::AllocationFailure {
            resource,
            requested,
        })
}

fn relation_check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricRelationError> {
    if requested > limit {
        Err(ParametricRelationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConcreteIntegralKey(Vec<i64>);

impl ConcreteIntegralKey {
    /// Construct an exact concrete integral key from unshifted integer powers.
    pub fn try_new(powers: impl IntoIterator<Item = i64>) -> Result<Self, ParametricRelationError> {
        let mut retained = Vec::new();
        for power in powers {
            try_reserve_relation_entries("concrete integral powers", &mut retained, 1)?;
            retained.push(power);
        }
        if retained.is_empty() {
            return Err(ParametricRelationError::EmptyIndexSpace);
        }
        Ok(Self(retained))
    }

    fn checked_from_assignment(
        assignment: &[i64],
        shift: &IndexShift,
    ) -> Result<Self, ParametricRelationError> {
        if assignment.len() != shift.arity() {
            return Err(ParametricRelationError::WrongArity {
                expected: shift.arity(),
                actual: assignment.len(),
            });
        }
        let mut powers = Vec::new();
        try_reserve_relation_entries("concrete integral powers", &mut powers, assignment.len())?;
        for (position, (&power, &offset)) in assignment.iter().zip(shift.values()).enumerate() {
            powers.push(
                power
                    .checked_add(offset)
                    .ok_or(ParametricRelationError::IndexOverflow { position })?,
            );
        }
        Ok(Self(powers))
    }

    pub fn powers(&self) -> &[i64] {
        &self.0
    }

    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        self.0.capacity().checked_mul(size_of::<i64>())
    }
}

#[derive(Clone, Debug)]
pub struct ConcreteRelation {
    family_fingerprint: Arc<str>,
    row_id: ParametricRowId,
    arity: usize,
    terms: BTreeMap<ConcreteIntegralKey, Coefficient>,
    // Compatibility polynomial-only view. `guarded_nonzero` is canonical.
    nonzero: Vec<BasePolynomial>,
    guarded_nonzero: Vec<SpecializedNonZeroCondition>,
}

impl PartialEq for ConcreteRelation {
    fn eq(&self, other: &Self) -> bool {
        self.family_fingerprint == other.family_fingerprint
            && self.row_id == other.row_id
            && self.arity == other.arity
            && self.terms == other.terms
            && self.nonzero == other.nonzero
    }
}

impl Eq for ConcreteRelation {}

impl ConcreteRelation {
    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub fn row_id(&self) -> &ParametricRowId {
        &self.row_id
    }

    pub fn terms(&self) -> &BTreeMap<ConcreteIntegralKey, Coefficient> {
        &self.terms
    }

    pub fn nonzero_conditions(&self) -> &[BasePolynomial] {
        &self.nonzero
    }

    pub fn guarded_nonzero_conditions(&self) -> &[SpecializedNonZeroCondition] {
        &self.guarded_nonzero
    }

    /// Conservative bytes owned by a deep clone of this concrete relation.
    /// Family and row-label `Arc` payloads remain authenticated sharing seams;
    /// all sparse maps, keys, coefficients, guard polynomials, origins, and
    /// backing-vector capacities are charged.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let btree_node_bound = size_of::<(ConcreteIntegralKey, Coefficient)>()
            .checked_mul(16)?
            .checked_add(32usize.checked_mul(size_of::<usize>())?)?;
        let mut bytes = size_of::<Self>();
        for (key, coefficient) in &self.terms {
            bytes = bytes.checked_add(btree_node_bound)?;
            bytes = bytes.checked_add(key.owned_retained_byte_bound()?)?;
            bytes = bytes.checked_add(coefficient_owned_retained_byte_bound(coefficient)?)?;
        }
        bytes = bytes.checked_add(
            self.nonzero
                .capacity()
                .checked_mul(size_of::<BasePolynomial>())?,
        )?;
        for polynomial in &self.nonzero {
            bytes = bytes.checked_add(polynomial.owned_retained_byte_bound()?)?;
        }
        bytes = bytes.checked_add(
            self.guarded_nonzero
                .capacity()
                .checked_mul(size_of::<SpecializedNonZeroCondition>())?,
        )?;
        for condition in &self.guarded_nonzero {
            bytes = bytes.checked_add(condition.owned_retained_byte_bound()?)?;
        }
        Some(bytes)
    }

    /// Seal every concrete guard before a generated-affine relation crosses
    /// the public reduction boundary.
    ///
    /// Exact polynomials and their canonical order are preserved. Complete
    /// provenance remains replayable through the retained generated-affine
    /// owner certificate; only the public origin sets are redacted here.
    pub(crate) fn seal_generated_affine_guard_provenance(&mut self) {
        for condition in &mut self.guarded_nonzero {
            condition.seal_generated_affine_provenance();
        }
    }

    pub fn has_identical_guard_provenance(&self, other: &Self) -> bool {
        self == other && self.guarded_nonzero == other.guarded_nonzero
    }

    fn add_guarded_nonzero(
        &mut self,
        context: &CoefficientContext,
        mut condition: SpecializedNonZeroCondition,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        if !context.contains(&condition.polynomial().raw().clone().into()) {
            return Err(ParametricRelationError::WrongContext);
        }
        if condition.polynomial().is_zero() {
            return Err(ParametricRelationError::UnsatisfiableDomain);
        }
        if condition.polynomial().is_nonzero_constant() {
            return Ok(());
        }
        condition.add_origin_with_limit(
            GuardOrigin::RelationConditionAttached {
                row: self.row_id.guard_identity(),
            },
            limits.max_guard_origins,
        )?;
        let is_new = !self
            .guarded_nonzero
            .iter()
            .any(|existing| existing.polynomial() == condition.polynomial());
        let polynomial = condition.polynomial().clone();
        insert_specialized_condition(
            &mut self.guarded_nonzero,
            condition,
            limits.max_guard_origins,
        )?;
        if is_new {
            self.nonzero.push(polynomial);
        }
        Ok(())
    }

    fn add_term(
        &mut self,
        context: &CoefficientContext,
        key: ConcreteIntegralKey,
        coefficient: Coefficient,
        limits: ParametricArithmeticLimits,
    ) -> Result<(), ParametricRelationError> {
        if key.powers().len() != self.arity {
            return Err(ParametricRelationError::WrongArity {
                expected: self.arity,
                actual: key.powers().len(),
            });
        }
        if !context.contains(&coefficient) {
            return Err(ParametricRelationError::WrongContext);
        }
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = self.terms.get(&key) {
            let sum = context
                .try_add(current, &coefficient, limits.exact_algebra)
                .map_err(crate::ParametricCoefficientError::from)?;
            if sum.is_zero() {
                self.terms.remove(&key);
            } else {
                self.terms.insert(key, sum);
            }
        } else {
            self.terms.insert(key, coefficient);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRelationError {
    EmptyIndexSpace,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    IndexOutOfRange {
        position: usize,
        arity: usize,
    },
    IndexOverflow {
        position: usize,
    },
    WrongContext,
    WrongFamily,
    UnsatisfiableDomain,
    PrevalidatedTermCollision,
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
    ManifestEncodingMismatch {
        expected_bytes: usize,
        actual_bytes: usize,
    },
    SpecializationReplayMismatch,
    Coefficient(ParametricCoefficientError),
}

impl fmt::Display for ParametricRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => formatter.write_str("an integral index space cannot be empty"),
            Self::WrongArity { expected, actual } => {
                write!(formatter, "index arity is {actual}, expected {expected}")
            }
            Self::IndexOutOfRange { position, arity } => {
                write!(
                    formatter,
                    "index position {position} is outside arity {arity}"
                )
            }
            Self::IndexOverflow { position } => {
                write!(formatter, "integer index overflow at position {position}")
            }
            Self::WrongContext => formatter.write_str("relation and coefficient contexts differ"),
            Self::WrongFamily => formatter.write_str("relations belong to different families"),
            Self::UnsatisfiableDomain => {
                formatter.write_str("relation domain contains an identically zero nonzero guard")
            }
            Self::PrevalidatedTermCollision => formatter.write_str(
                "two supposedly distinct prevalidated relation terms have the same shift",
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} units for {resource}"
            ),
            Self::ManifestEncodingMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "parametric relation manifest counted {expected_bytes} bytes but wrote {actual_bytes}"
            ),
            Self::SpecializationReplayMismatch => formatter
                .write_str("partial parametric relation specialization did not replay identically"),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ParametricRelationError {}

impl From<ParametricCoefficientError> for ParametricRelationError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_affine_top_reduction_origin_observes_every_locator_field() {
        #[derive(Default)]
        struct UnsignedObserver(Vec<u128>);

        impl ParametricRelationV2Observer for UnsignedObserver {
            fn observe_unsigned(&mut self, value: u128) -> fmt::Result {
                self.0.push(value);
                Ok(())
            }
        }

        let origin = GuardOrigin::GeneratedAffineGroupTopReductionCoefficientDenominator {
            solve_group_ordinal: 17,
            database_epoch: 23,
            event_ordinal: 31,
            operation_ordinal: 37,
            term_ordinal: 41,
            pivot_normalization: true,
        };
        let mut observer = UnsignedObserver::default();
        observe_guard_origin(&mut observer, &origin).unwrap();
        assert_eq!(observer.0, [17, 23, 31, 37, 41, 1]);
    }

    fn concrete_specialization_plan_fixture() -> (
        ParametricCoefficientContext,
        ParametricRelation,
        Vec<ParametricNonZeroCondition>,
        [i64; 2],
    ) {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(
            &base,
            "private-concrete-specialization-plan-context",
            2,
        )
        .unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();

        let n0_squared = context.mul(&n0, &n0).unwrap();
        let x_n1 = context.mul(&x, &n1).unwrap();
        let numerator = context
            .add(
                &context.add(&n0_squared, &x_n1).unwrap(),
                &context.integer(7),
            )
            .unwrap();
        let n0_n1 = context.mul(&n0, &n1).unwrap();
        let denominator = context
            .add(&context.add(&n0_n1, &x).unwrap(), &context.integer(3))
            .unwrap();
        let rational = context.checked_div(&numerator, &denominator).unwrap();
        let second_coefficient = context.add(&context.mul(&x, &n0).unwrap(), &n1).unwrap();

        let row_id = ParametricRowId::Derived {
            label: Arc::from("private-concrete-specialization-plan-row"),
        };
        let mut relation = ParametricRelation::new(
            "private-concrete-specialization-plan-family",
            row_id,
            &context,
        );
        relation.add_term(&context, space.zero(), rational).unwrap();
        relation
            .add_term(&context, space.shift([1, -2]).unwrap(), second_coefficient)
            .unwrap();

        let guard_value = context.add(&context.add(&n0, &n1).unwrap(), &x).unwrap();
        let guard_polynomial = context.numerator_condition(&guard_value).unwrap();
        let own_guard = context
            .nonzero_condition(
                guard_polynomial.clone(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, own_guard)
            .unwrap();
        let additional_guard = context
            .nonzero_condition(
                guard_polynomial,
                GuardOrigin::GuardedDivisionDividendDenominator,
            )
            .unwrap();

        (context, relation, vec![additional_guard], [4, 5])
    }

    fn exact_concrete_specialization_limits(
        stats: ParametricConcreteSpecializationPreflight,
    ) -> ParametricConcreteSpecializationLimits {
        ParametricConcreteSpecializationLimits {
            arithmetic: ParametricArithmeticLimits::default(),
            max_source_terms: stats.source_terms(),
            max_source_exponent_entries: stats.source_exponent_entries(),
            max_output_term_bound: stats.output_term_bound(),
            max_output_exponent_entry_bound: stats.output_exponent_entry_bound(),
            max_power_operation_bound: stats.power_operation_bound(),
            max_integer_bit_work_bound: stats.integer_bit_work_bound(),
            max_normalization_input_term_pair_bound: stats.normalization_input_term_pair_bound(),
            max_key_component_bound: stats.key_component_bound(),
            max_guard_occurrence_bound: stats.guard_occurrence_bound(),
            max_guard_polynomial_retained_byte_bound: stats.guard_polynomial_retained_byte_bound(),
            max_guard_origin_occurrence_bound: stats.guard_origin_occurrence_bound(),
            max_guard_origin_retained_byte_bound: stats.guard_origin_retained_byte_bound(),
            max_normalized_coefficient_term_bound: stats.normalized_coefficient_term_bound(),
            max_normalized_coefficient_retained_byte_bound: stats
                .normalized_coefficient_retained_byte_bound(),
            max_concrete_relation_retained_byte_bound: stats
                .concrete_relation_retained_byte_bound(),
            max_peak_execution_retained_byte_bound: stats.peak_execution_retained_byte_bound(),
        }
    }

    #[test]
    fn translation_moves_keys_and_coefficient_indices_together() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "relation-translate", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        relation
            .add_term(&context, space.zero(), context.index(0).unwrap())
            .unwrap();
        let translation = space.shift([2, -1]).unwrap();
        let translated = relation
            .translated(
                &context,
                &translation,
                ParametricRowId::Derived {
                    label: "translated".into(),
                },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let concrete = translated
            .specialize(&context, &[3, 7], ParametricArithmeticLimits::default())
            .unwrap();
        let (key, coefficient) = concrete.terms().first_key_value().unwrap();
        assert_eq!(key.powers(), &[5, 6]);
        assert_eq!(coefficient, &base.integer(5));
    }

    #[test]
    fn translation_composes_exactly() {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(&base, "relation-compose", 2).unwrap();
        let space = IndexSpace::try_new(2).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term(
                &context,
                space.unit(1, 1).unwrap(),
                context.index(0).unwrap(),
            )
            .unwrap();
        let s = space.shift([1, -2]).unwrap();
        let t = space.shift([-4, 3]).unwrap();
        let st = s.checked_add(&t).unwrap();
        let sequential = source
            .translated(
                &context,
                &s,
                ParametricRowId::Derived { label: "s".into() },
                ParametricArithmeticLimits::default(),
            )
            .unwrap()
            .translated(
                &context,
                &t,
                ParametricRowId::Derived { label: "st".into() },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let direct = source
            .translated(
                &context,
                &st,
                ParametricRowId::Derived { label: "st".into() },
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(sequential.terms(), direct.terms());
        assert_eq!(sequential.nonzero_conditions(), direct.nonzero_conditions());
    }

    #[test]
    fn specialization_checks_key_overflow() {
        let base = CoefficientContext::new(["d"]);
        let context = ParametricCoefficientContext::try_new(&base, "relation-overflow", 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "source".into(),
            },
            &context,
        );
        source
            .add_term(&context, space.unit(0, 1).unwrap(), context.one())
            .unwrap();
        assert!(matches!(
            source.specialize(&context, &[i64::MAX], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
    }

    #[test]
    fn borrowed_additional_conditions_are_checked_before_terms() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "borrowed-specialization-guard", 1)
                .unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "borrowed-specialization-source".into(),
            },
            &context,
        );
        source
            .add_term(&context, space.unit(0, 1).unwrap(), context.one())
            .unwrap();

        let borrowed_polynomial = context
            .sub(&context.index(0).unwrap(), &context.integer(i64::MAX))
            .and_then(|value| context.numerator_condition(&value))
            .unwrap();
        let mut condition_owner = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: "borrowed-specialization-condition-owner".into(),
            },
            &context,
        );
        condition_owner
            .add_nonzero_condition(&context, borrowed_polynomial)
            .unwrap();
        let borrowed_base_polynomial = context
            .lift(&base.parameter("d").unwrap())
            .and_then(|value| context.numerator_condition(&value))
            .unwrap();
        condition_owner
            .add_nonzero_condition(&context, borrowed_base_polynomial)
            .unwrap();

        let limits = ParametricArithmeticLimits::default();
        assert!(matches!(
            source.specialize(&context, &[i64::MAX], limits),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
        assert!(matches!(
            source.specialize_with_additional_nonzero_conditions(
                &context,
                &[i64::MAX],
                condition_owner.guarded_nonzero_conditions(),
                limits,
            ),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));

        let ordinary = source.specialize(&context, &[0], limits).unwrap();
        let borrowed = source
            .specialize_with_additional_nonzero_conditions(
                &context,
                &[0],
                condition_owner.guarded_nonzero_conditions(),
                limits,
            )
            .unwrap();
        assert_eq!(borrowed.terms(), ordinary.terms());
        assert_eq!(ordinary.nonzero_conditions().len(), 0);
        assert_eq!(borrowed.guarded_nonzero_conditions().len(), 1);
        assert!(borrowed.guarded_nonzero_conditions()[0].origins().contains(
            &GuardOrigin::RelationConditionAttached {
                row: source.row_id().guard_identity(),
            }
        ));
    }

    #[test]
    fn concrete_specialization_plan_matches_backend_and_covers_observed_storage() {
        let (context, relation, additional, assignment) = concrete_specialization_plan_fixture();
        let limits = ParametricConcreteSpecializationLimits::default();
        let plan = relation
            .prepare_concrete_specialization_with_additional_nonzero_conditions(
                &context,
                &assignment,
                &additional,
                limits,
            )
            .unwrap();
        let stats = plan.preflight();
        let debug = format!("{plan:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("private-concrete-specialization-plan-row"));
        assert!(!debug.contains("private-concrete-specialization-plan-family"));
        assert!(!debug.contains("private-concrete-specialization-plan-context"));

        let direct = relation
            .specialize_with_additional_nonzero_conditions(
                &context,
                &assignment,
                additional.iter(),
                limits.arithmetic,
            )
            .unwrap();
        let planned = plan.execute().unwrap();
        assert_eq!(planned.terms(), direct.terms());
        assert_eq!(planned.nonzero_conditions(), direct.nonzero_conditions());
        assert_eq!(
            planned.guarded_nonzero_conditions(),
            direct.guarded_nonzero_conditions()
        );
        assert!(
            planned.owned_retained_byte_bound().unwrap()
                <= stats.concrete_relation_retained_byte_bound()
        );
        assert!(
            stats.peak_execution_retained_byte_bound()
                >= stats.concrete_relation_retained_byte_bound()
        );
        assert_eq!(stats.key_component_bound(), 4);
        // Two source guards (the explicit relation guard and the rational
        // term's pre-cancellation denominator), one borrowed guard, and one
        // prospective denominator occurrence for each of the two terms.
        assert_eq!(stats.guard_occurrence_bound(), 5);
    }

    #[test]
    fn concrete_specialization_plan_accepts_exact_aggregate_limits() {
        let (context, relation, additional, assignment) = concrete_specialization_plan_fixture();
        let stats = relation
            .prepare_concrete_specialization_with_additional_nonzero_conditions(
                &context,
                &assignment,
                &additional,
                ParametricConcreteSpecializationLimits::default(),
            )
            .unwrap()
            .preflight();
        let exact = exact_concrete_specialization_limits(stats);
        let plan = relation
            .prepare_concrete_specialization_with_additional_nonzero_conditions(
                &context,
                &assignment,
                &additional,
                exact,
            )
            .unwrap();
        assert_eq!(plan.preflight(), stats);
        let concrete = plan.execute().unwrap();
        assert!(
            concrete.owned_retained_byte_bound().unwrap()
                <= stats.concrete_relation_retained_byte_bound()
        );
    }

    #[test]
    fn concrete_specialization_plan_rejects_every_aggregate_limit_one_below() {
        let (context, relation, additional, assignment) = concrete_specialization_plan_fixture();
        let stats = relation
            .prepare_concrete_specialization_with_additional_nonzero_conditions(
                &context,
                &assignment,
                &additional,
                ParametricConcreteSpecializationLimits::default(),
            )
            .unwrap()
            .preflight();

        macro_rules! assert_one_below {
            ($limit_field:ident, $stats_getter:ident, $resource:literal) => {{
                let requested = stats.$stats_getter();
                assert!(requested > 0, "fixture must exercise {}", $resource);
                let mut limits = exact_concrete_specialization_limits(stats);
                limits.$limit_field = requested - 1;
                assert!(matches!(
                    relation.prepare_concrete_specialization_with_additional_nonzero_conditions(
                        &context,
                        &assignment,
                        &additional,
                        limits,
                    ),
                    Err(ParametricRelationError::ResourceLimit {
                        resource: observed_resource,
                        requested: observed_requested,
                        limit,
                    }) if observed_resource == $resource
                        && observed_requested == requested
                        && limit == requested - 1
                ));
            }};
        }

        assert_one_below!(
            max_source_terms,
            source_terms,
            "concrete specialization source terms"
        );
        assert_one_below!(
            max_source_exponent_entries,
            source_exponent_entries,
            "concrete specialization source exponent entries"
        );
        assert_one_below!(
            max_output_term_bound,
            output_term_bound,
            "concrete specialization output terms"
        );
        assert_one_below!(
            max_output_exponent_entry_bound,
            output_exponent_entry_bound,
            "concrete specialization output exponent entries"
        );
        assert_one_below!(
            max_power_operation_bound,
            power_operation_bound,
            "concrete specialization power operations"
        );
        assert_one_below!(
            max_integer_bit_work_bound,
            integer_bit_work_bound,
            "concrete specialization integer-bit work"
        );
        assert_one_below!(
            max_normalization_input_term_pair_bound,
            normalization_input_term_pair_bound,
            "concrete specialization normalization input term pairs"
        );
        assert_one_below!(
            max_key_component_bound,
            key_component_bound,
            "concrete specialization key components"
        );
        assert_one_below!(
            max_guard_occurrence_bound,
            guard_occurrence_bound,
            "concrete specialization guard occurrences"
        );
        assert_one_below!(
            max_guard_polynomial_retained_byte_bound,
            guard_polynomial_retained_byte_bound,
            "concrete specialization guard-polynomial retained bytes"
        );
        assert_one_below!(
            max_guard_origin_occurrence_bound,
            guard_origin_occurrence_bound,
            "concrete specialization guard-origin occurrences"
        );
        assert_one_below!(
            max_guard_origin_retained_byte_bound,
            guard_origin_retained_byte_bound,
            "concrete specialization guard-origin retained bytes"
        );
        assert_one_below!(
            max_normalized_coefficient_term_bound,
            normalized_coefficient_term_bound,
            "concrete specialization normalized coefficient terms"
        );
        assert_one_below!(
            max_normalized_coefficient_retained_byte_bound,
            normalized_coefficient_retained_byte_bound,
            "concrete specialization normalized coefficient retained bytes"
        );
        assert_one_below!(
            max_concrete_relation_retained_byte_bound,
            concrete_relation_retained_byte_bound,
            "concrete specialization prospective relation bytes"
        );
        assert_one_below!(
            max_peak_execution_retained_byte_bound,
            peak_execution_retained_byte_bound,
            "concrete specialization peak visible retained bytes"
        );

        let requested = stats.guard_origin_occurrence_bound();
        let mut limits = exact_concrete_specialization_limits(stats);
        limits.arithmetic.max_guard_origins = requested - 1;
        assert!(matches!(
            relation.prepare_concrete_specialization_with_additional_nonzero_conditions(
                &context,
                &assignment,
                &additional,
                limits,
            ),
            Err(ParametricRelationError::ResourceLimit {
                resource: "concrete specialization merged guard-origin occurrences",
                requested: observed_requested,
                limit,
            }) if observed_requested == requested && limit == requested - 1
        ));
    }

    #[test]
    fn prevalidated_nonzero_condition_retains_the_supplied_origin_set() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "prevalidated-condition-parts", 1)
                .unwrap();
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let origins = std::collections::BTreeSet::from([
            GuardOrigin::GuardedDivisionDividendDenominator,
            GuardOrigin::GuardedDivisionDivisorNumerator,
        ]);
        let expected = origins.clone();
        let condition = context
            .nonzero_condition_from_prevalidated_parts(
                polynomial.clone(),
                origins,
                crate::algebra::ExactAlgebraLimits::default(),
                2,
            )
            .unwrap();
        assert_eq!(condition.origins(), &expected);

        assert!(matches!(
            context.nonzero_condition_from_prevalidated_parts(
                polynomial,
                expected,
                crate::algebra::ExactAlgebraLimits::default(),
                1,
            ),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "parametric guard origins",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn specialization_discards_zero_coefficient_before_key_arithmetic() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "zero-before-key-overflow", 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();
        let coefficient = context
            .sub(&context.index(0).unwrap(), &context.integer(i64::MAX))
            .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("zero-before-key-overflow"),
            },
            &context,
        );
        relation
            .add_term(&context, space.unit(0, 1).unwrap(), coefficient)
            .unwrap();

        let specialized = relation
            .specialize(&context, &[i64::MAX], ParametricArithmeticLimits::default())
            .unwrap();
        assert!(specialized.terms().is_empty());
    }

    #[test]
    fn equal_guard_polynomials_merge_deterministic_origin_sets() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guard-merge", 1).unwrap();
        let row_id = ParametricRowId::Derived {
            label: Arc::from("guard-source"),
        };
        let mut relation = ParametricRelation::new("family", row_id.clone(), &context);
        let n = context.index(0).unwrap();
        let polynomial = context.numerator_condition(&n).unwrap();
        let first = context
            .nonzero_condition(
                polynomial.clone(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        let second = context
            .nonzero_condition(polynomial, GuardOrigin::GuardedDivisionDividendDenominator)
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, first)
            .unwrap();
        relation
            .add_guarded_nonzero_condition(&context, second)
            .unwrap();

        assert_eq!(relation.nonzero_conditions().len(), 1);
        assert_eq!(relation.guarded_nonzero_conditions().len(), 1);
        assert_eq!(
            relation.guarded_nonzero_conditions()[0].origins(),
            &std::collections::BTreeSet::from([
                GuardOrigin::GuardedDivisionDividendDenominator,
                GuardOrigin::GuardedDivisionDivisorNumerator,
                GuardOrigin::RelationConditionAttached {
                    row: row_id.guard_identity(),
                },
            ])
        );
    }

    #[test]
    fn translation_and_specialization_preserve_and_augment_guard_origins() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "guard-map", 1).unwrap();
        let source_id = ParametricRowId::Derived {
            label: Arc::from("source-row"),
        };
        let target_id = ParametricRowId::Derived {
            label: Arc::from("translated-row"),
        };
        let mut source = ParametricRelation::new("family", source_id.clone(), &context);
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();
        let x_plus_n = context.add(&x, &context.index(0).unwrap()).unwrap();
        let polynomial = context.numerator_condition(&x_plus_n).unwrap();
        let condition = context
            .nonzero_condition(polynomial, GuardOrigin::GuardedDivisionDivisorNumerator)
            .unwrap()
            .try_with_origin(
                GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 47,
                    database_epoch: 53,
                    event_ordinal: 59,
                },
                ParametricArithmeticLimits::default().max_guard_origins,
            )
            .unwrap();
        source
            .add_guarded_nonzero_condition(&context, condition)
            .unwrap();

        let translation = IndexShift::try_new([2], 1).unwrap();
        let translated = source
            .translated(
                &context,
                &translation,
                target_id.clone(),
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let translated_guard = &translated.guarded_nonzero_conditions()[0];
        assert!(
            translated_guard
                .origins()
                .contains(&GuardOrigin::GuardedDivisionDivisorNumerator)
        );
        assert!(
            translated_guard
                .origins()
                .contains(&GuardOrigin::IndexTranslation {
                    offset: vec![2].into_boxed_slice(),
                })
        );
        assert!(
            translated_guard
                .origins()
                .contains(&GuardOrigin::RelationTranslation {
                    source_row: source_id.guard_identity(),
                    target_row: target_id.guard_identity(),
                    offset: vec![2].into_boxed_slice(),
                })
        );

        let mut concrete = translated
            .specialize(&context, &[3], ParametricArithmeticLimits::default())
            .unwrap();
        assert_eq!(concrete.guarded_nonzero_conditions().len(), 1);
        let specialized = &concrete.guarded_nonzero_conditions()[0];
        assert_eq!(
            specialized.polynomial().to_expression(),
            base.parse("x+5").unwrap().to_expression()
        );
        assert!(
            specialized
                .origins()
                .contains(&GuardOrigin::IndexSpecialization {
                    assignment: vec![3].into_boxed_slice(),
                })
        );
        assert!(
            specialized
                .origins()
                .contains(&GuardOrigin::GeneratedAffineGroupRecentering {
                    solve_group_ordinal: 47,
                    database_epoch: 53,
                    event_ordinal: 59,
                })
        );
        assert!(
            specialized
                .origins()
                .contains(&GuardOrigin::RelationConditionAttached {
                    row: source_id.guard_identity(),
                })
        );
        assert!(
            specialized
                .origins()
                .contains(&GuardOrigin::RelationConditionAttached {
                    row: target_id.guard_identity(),
                })
        );

        let expected_nonzero = concrete.nonzero_conditions().to_vec();
        let expected_polynomial = concrete.guarded_nonzero_conditions()[0]
            .polynomial()
            .clone();
        concrete.seal_generated_affine_guard_provenance();
        assert_eq!(concrete.nonzero_conditions(), expected_nonzero);
        assert_eq!(
            concrete.guarded_nonzero_conditions()[0].polynomial(),
            &expected_polynomial
        );
        assert_eq!(
            concrete.guarded_nonzero_conditions()[0].origins(),
            &std::collections::BTreeSet::from([GuardOrigin::GeneratedAffineSealedCondition])
        );
    }

    #[test]
    fn group_recentering_locator_preserves_ordinary_relation_equality() {
        let base = CoefficientContext::new(["x"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "group-recentering-locator-equality", 1)
                .unwrap();
        let row = ParametricRowId::Derived {
            label: Arc::from("same-row"),
        };
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();

        let mut left = ParametricRelation::new("same-family", row.clone(), &context);
        left.add_guarded_nonzero_condition(
            &context,
            context
                .nonzero_condition(
                    polynomial.clone(),
                    GuardOrigin::GeneratedAffineGroupRecentering {
                        solve_group_ordinal: 1,
                        database_epoch: 2,
                        event_ordinal: 3,
                    },
                )
                .unwrap(),
        )
        .unwrap();

        let mut right = ParametricRelation::new("same-family", row, &context);
        right
            .add_guarded_nonzero_condition(
                &context,
                context
                    .nonzero_condition(
                        polynomial,
                        GuardOrigin::GeneratedAffineGroupRecentering {
                            solve_group_ordinal: 1,
                            database_epoch: 2,
                            event_ordinal: 4,
                        },
                    )
                    .unwrap(),
            )
            .unwrap();

        assert_eq!(left, right);
        assert!(!left.has_identical_guard_provenance(&right));
        assert_ne!(left.stable_manifest(), right.stable_manifest());
    }

    #[test]
    fn guarded_zero_division_term_keeps_its_domain() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "zero-div-term", 1).unwrap();
        let n = context.index(0).unwrap();
        let divided = context.checked_div_guarded(&context.zero(), &n).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("guarded-zero"),
            },
            &context,
        );
        relation
            .add_guarded_term(&context, IndexSpace::try_new(1).unwrap().zero(), divided)
            .unwrap();
        assert!(relation.terms().is_empty());
        assert_eq!(relation.guarded_nonzero_conditions().len(), 1);
        assert!(matches!(
            relation.specialize(&context, &[0], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));
        assert!(
            relation
                .specialize(&context, &[1], ParametricArithmeticLimits::default())
                .unwrap()
                .terms()
                .is_empty()
        );
    }

    #[test]
    fn custom_guard_origin_limit_is_enforced_at_relation_boundary() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guard-limit", 1).unwrap();
        let n = context.index(0).unwrap();
        let condition = context
            .nonzero_condition(
                context.numerator_condition(&n).unwrap(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("limited"),
            },
            &context,
        );
        let limits = ParametricArithmeticLimits {
            max_guard_origins: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            relation.add_guarded_nonzero_condition_with_limits(&context, condition, limits),
            Err(ParametricRelationError::Coefficient(
                ParametricCoefficientError::ResourceLimit {
                    resource: "parametric guard origins",
                    requested: 2,
                    limit: 1,
                }
            ))
        ));
    }

    #[test]
    fn guarded_scaled_addition_retains_a_zero_factor_domain() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guarded-scale", 1).unwrap();
        let row_id = ParametricRowId::Derived {
            label: Arc::from("scale-row"),
        };
        let mut source = ParametricRelation::new("family", row_id.clone(), &context);
        source
            .add_term(
                &context,
                IndexSpace::try_new(1).unwrap().zero(),
                context.one(),
            )
            .unwrap();
        let factor = context
            .checked_div_guarded(&context.zero(), &context.index(0).unwrap())
            .unwrap();
        let mut target = ParametricRelation::new("family", row_id, &context);
        target
            .add_scaled_guarded(&context, &source, factor)
            .unwrap();

        assert!(target.terms().is_empty());
        assert_eq!(target.guarded_nonzero_conditions().len(), 1);
        assert!(matches!(
            target.specialize(&context, &[0], ParametricArithmeticLimits::default()),
            Err(ParametricRelationError::UnsatisfiableDomain)
        ));
    }

    #[test]
    fn typed_v2_manifest_is_deterministic_injective_and_exactly_bounded() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "manifest-v2", 1).unwrap();
        let space = IndexSpace::try_new(1).unwrap();

        let build = || {
            let n = context.index(0).unwrap();
            let numerator = context
                .add(&context.mul(&n, &n).unwrap(), &context.integer(-5))
                .unwrap();
            let denominator = context.add(&n, &context.integer(3)).unwrap();
            let coefficient = context.checked_div(&numerator, &denominator).unwrap();
            let mut relation = ParametricRelation::new(
                "family",
                ParametricRowId::Derived {
                    label: Arc::from("manifest-row"),
                },
                &context,
            );
            relation
                .add_term(&context, space.zero(), coefficient)
                .unwrap();
            let guard = context.add(&n, &context.integer(7)).unwrap();
            relation
                .add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
                .unwrap();
            relation
        };

        let first = build();
        let independently_allocated_equal = build();
        let manifest = first.stable_manifest();
        assert!(manifest.starts_with(PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA));
        assert_eq!(manifest, independently_allocated_equal.stable_manifest());
        assert_eq!(
            first.stable_manifest_with_limit(manifest.len()).unwrap(),
            manifest
        );
        let one_below = manifest.len() - 1;
        assert!(matches!(
            first.stable_manifest_with_limit(one_below),
            Err(ParametricRelationError::ResourceLimit {
                resource: "parametric relation manifest bytes",
                requested,
                limit,
            }) if requested > limit && limit == one_below
        ));

        // Adversarially exchange numerator/denominator, then change only the
        // sparse exponent structure. Both changes must alter the transcript.
        let n = context.index(0).unwrap();
        let numerator = context
            .add(&context.mul(&n, &n).unwrap(), &context.integer(-5))
            .unwrap();
        let denominator = context.add(&n, &context.integer(3)).unwrap();
        let mut exchanged = ParametricRelation::new("family", first.row_id().clone(), &context);
        exchanged
            .add_term(
                &context,
                space.zero(),
                context.checked_div(&denominator, &numerator).unwrap(),
            )
            .unwrap();
        assert_ne!(manifest, exchanged.stable_manifest());

        let mut exponent_changed =
            ParametricRelation::new("family", first.row_id().clone(), &context);
        exponent_changed
            .add_term(
                &context,
                space.zero(),
                context
                    .checked_div(
                        &context.add(&n, &context.integer(-5)).unwrap(),
                        &denominator,
                    )
                    .unwrap(),
            )
            .unwrap();
        assert_ne!(manifest, exponent_changed.stable_manifest());
    }

    #[test]
    fn length_prefix_subpass_uses_the_observer_finite_ceiling() {
        struct FinitePrefixObserver {
            exceeded: Option<(usize, usize)>,
        }

        impl ParametricRelationV2Observer for FinitePrefixObserver {
            fn length_prefix_byte_limit(&self) -> usize {
                3
            }

            fn observe_length_prefix_limit_exceeded(
                &mut self,
                local_requested: usize,
                local_limit: usize,
            ) -> fmt::Result {
                self.exceeded = Some((local_requested, local_limit));
                Err(fmt::Error)
            }
        }

        let mut output = String::new();
        let mut observer = FinitePrefixObserver { exceeded: None };
        assert!(
            write_length_prefixed_manifest_payload(&mut output, "four", &mut observer).is_err()
        );
        assert_eq!(observer.exceeded, Some((4, 3)));
        assert!(output.is_empty());
    }

    #[test]
    fn partial_specialization_shares_source_and_assumption_storage() {
        let base = CoefficientContext::new(["theta"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "shared-partial-source", 1).unwrap();
        let n = context.index(0).unwrap();
        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let denominator = context.add(&n, &theta).unwrap();
        let mut relation = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("shared-partial-source"),
            },
            &context,
        );
        relation
            .add_term(
                &context,
                IndexSpace::try_new(1).unwrap().zero(),
                context.checked_div(&context.one(), &denominator).unwrap(),
            )
            .unwrap();
        let source = Arc::new(relation);
        let assignment = PartialIndexAssignment::try_new([(0, 0)], 1, 1).unwrap();
        let mut limits = PartialParametricRelationSpecializationLimits::default();
        limits.max_base_assumptions = 1;
        let certificate = source
            .partially_specialized_on(&context, assignment.clone(), limits)
            .unwrap();
        assert!(Arc::ptr_eq(&source, &certificate.source));
        assert_eq!(certificate.base_assumptions().len(), 1);
        let cloned = certificate.clone();
        assert!(Arc::ptr_eq(
            &certificate.base_assumptions,
            &cloned.base_assumptions
        ));

        limits.max_base_assumptions = 0;
        assert!(matches!(
            source.partially_specialized_on(&context, assignment, limits),
            Err(ParametricRelationError::ResourceLimit {
                resource: "partial relation base assumptions",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn fallible_index_and_integral_key_construction_preserves_checked_semantics() {
        let space = IndexSpace::try_new(2).unwrap();
        assert_eq!(space.try_zero().unwrap().values(), &[0, 0]);
        assert_eq!(space.unit(1, -1).unwrap().values(), &[0, -1]);
        assert_eq!(
            IndexShift::try_new([2, -3], 2)
                .unwrap()
                .checked_add(&IndexShift::try_new([-1, 5], 2).unwrap())
                .unwrap()
                .values(),
            &[1, 2]
        );
        assert!(matches!(
            IndexShift::try_new([i64::MAX], 1)
                .unwrap()
                .checked_add(&IndexShift::try_new([1], 1).unwrap()),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
        assert_eq!(
            ConcreteIntegralKey::try_new([3, -2]).unwrap().powers(),
            &[3, -2]
        );

        // This is rejected by Vec's capacity arithmetic without attempting a
        // material allocation, exercising the public allocation error path.
        assert!(matches!(
            IndexSpace::try_new(usize::MAX).unwrap().try_zero(),
            Err(ParametricRelationError::AllocationFailure {
                resource: "zero index-shift components",
                requested: usize::MAX,
            })
        ));
    }

    #[test]
    fn overlong_index_shift_is_rejected_without_draining_the_iterator() {
        struct PanicIfPolledAfterFirstExtra {
            polls: usize,
        }

        impl Iterator for PanicIfPolledAfterFirstExtra {
            type Item = i64;

            fn next(&mut self) -> Option<Self::Item> {
                self.polls += 1;
                match self.polls {
                    1 => Some(7),
                    2 => Some(11),
                    _ => panic!("an overlong shift iterator was drained after arity was known"),
                }
            }
        }

        assert_eq!(
            IndexShift::try_new(PanicIfPolledAfterFirstExtra { polls: 0 }, 1),
            Err(ParametricRelationError::WrongArity {
                expected: 1,
                actual: 2,
            })
        );
    }

    #[test]
    fn retained_byte_counter_distinguishes_usize_overflow_from_limit() {
        let mut overflow = RelationBoundedByteCounter {
            bytes: usize::MAX,
            limit: usize::MAX,
            failure: None,
        };
        assert!(overflow.write_str("x").is_err());
        assert_eq!(
            overflow.into_error(),
            ParametricRelationError::ResourceCountOverflow {
                resource: "partial relation retained bytes",
            }
        );

        let mut limited = RelationBoundedByteCounter {
            bytes: 1,
            limit: 1,
            failure: None,
        };
        assert!(limited.write_str("x").is_err());
        assert_eq!(
            limited.into_error(),
            ParametricRelationError::ResourceLimit {
                resource: "partial relation retained bytes",
                requested: 2,
                limit: 1,
            }
        );
    }

    #[test]
    fn affine_free_recentering_keeps_coefficient_and_key_translations_distinct() {
        let base = CoefficientContext::new(["d"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "split-affine-recentering", 2).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let two_n1 = context.mul(&context.integer(2), &n1).unwrap();
        let coefficient = context.add(&n0, &two_n1).unwrap();
        let source_id = ParametricRowId::Derived {
            label: Arc::from("split-affine-source"),
        };
        let target_id = ParametricRowId::Derived {
            label: Arc::from("split-affine-target"),
        };
        let mut source = ParametricRelation::new("family", source_id.clone(), &context);
        source
            .add_term(
                &context,
                IndexShift::try_new([5, -2], 2).unwrap(),
                coefficient,
            )
            .unwrap();
        source
            .add_term(
                &context,
                IndexShift::try_new([6, 4], 2).unwrap(),
                context.one(),
            )
            .unwrap();
        let d = context.lift(&base.parameter("d").unwrap()).unwrap();
        let guard_polynomial = context
            .numerator_condition(&context.add(&d, &n1).unwrap())
            .unwrap();
        let guard = context
            .nonzero_condition(
                guard_polynomial,
                GuardOrigin::GuardedDivisionDivisorNumerator,
            )
            .unwrap();
        source
            .add_guarded_nonzero_condition(&context, guard)
            .unwrap();
        for polynomial in [
            context.add(&d, &n0).unwrap(),
            context
                .add(&context.add(&n0, &n1).unwrap(), &context.integer(1))
                .unwrap(),
        ] {
            let polynomial = context.numerator_condition(&polynomial).unwrap();
            let guard = context
                .nonzero_condition(polynomial, GuardOrigin::GuardedDivisionDivisorNumerator)
                .unwrap();
            source
                .add_guarded_nonzero_condition(&context, guard)
                .unwrap();
        }

        let coefficient_translation = IndexShift::try_new([-2, 3], 2).unwrap();
        let key_center = IndexShift::try_new([5, -2], 2).unwrap();
        let (recentered, stats) = source
            .affine_free_recentered(
                &context,
                &coefficient_translation,
                &key_center,
                target_id.clone(),
                ParametricAffineFreeRecenteringLimits::default(),
            )
            .unwrap();

        let zero = IndexShift::try_new([0, 0], 2).unwrap();
        let displaced = IndexShift::try_new([1, 6], 2).unwrap();
        let expected = context
            .add(&context.add(&n0, &two_n1).unwrap(), &context.integer(4))
            .unwrap();
        assert_eq!(recentered.terms().get(&zero), Some(&expected));
        assert_eq!(recentered.terms().get(&displaced), Some(&context.one()));
        assert_eq!(recentered.guarded_nonzero_conditions().len(), 3);
        assert_eq!(recentered.nonzero_conditions().len(), 3);
        assert!(recentered.owned_retained_byte_bound().unwrap() <= stats.retained_bytes());
        let recentered_guard = &recentered.guarded_nonzero_conditions()[0];
        assert_eq!(
            recentered_guard.polynomial(),
            &context
                .numerator_condition(
                    &context
                        .add(&context.add(&d, &n1).unwrap(), &context.integer(3))
                        .unwrap(),
                )
                .unwrap()
        );
        assert!(
            recentered_guard
                .origins()
                .contains(&GuardOrigin::RelationAffineFreeRecentering {
                    source_row: source_id.guard_identity(),
                    target_row: target_id.guard_identity(),
                    coefficient_offset: vec![-2, 3],
                    key_center: vec![5, -2],
                })
        );

        let mut exact = ParametricAffineFreeRecenteringLimits::default();
        exact.max_terms = stats.terms();
        exact.max_guards = stats.guards();
        exact.max_translation_components = stats.translation_components();
        exact.max_key_subtraction_boundary_checks = stats.key_subtraction_boundary_checks();
        exact.max_source_terms = stats.source_terms();
        exact.max_source_exponent_entries = stats.source_exponent_entries();
        exact.max_output_terms = stats.output_terms();
        exact.max_output_exponent_entries = stats.output_exponent_entries();
        exact.max_power_operations = stats.power_operations();
        exact.max_integer_bit_work = stats.integer_bit_work();
        exact.max_normalized_coefficient_terms = stats.normalized_coefficient_terms();
        exact.max_retained_bytes = stats.retained_bytes();
        let (_, exact_stats) = source
            .affine_free_recentered(
                &context,
                &coefficient_translation,
                &key_center,
                target_id.clone(),
                exact,
            )
            .unwrap();
        assert_eq!(exact_stats, stats);

        let mut key_work_one_below = exact;
        key_work_one_below.max_key_subtraction_boundary_checks =
            stats.key_subtraction_boundary_checks() - 1;
        assert!(matches!(
            source.affine_free_recentered(
                &context,
                &coefficient_translation,
                &key_center,
                target_id.clone(),
                key_work_one_below,
            ),
            Err(ParametricRelationError::ResourceLimit {
                resource: "affine free recentering key-subtraction boundary checks",
                requested,
                limit,
            }) if requested == stats.key_subtraction_boundary_checks()
                && limit + 1 == requested
        ));

        let mut one_below = exact;
        one_below.max_retained_bytes = stats.retained_bytes() - 1;
        assert!(matches!(
            source.affine_free_recentered(
                &context,
                &coefficient_translation,
                &key_center,
                target_id,
                one_below,
            ),
            Err(ParametricRelationError::ResourceLimit {
                resource: "affine free recentering retained bytes",
                requested,
                limit,
            }) if requested == stats.retained_bytes() && limit + 1 == requested
        ));
    }

    #[test]
    fn affine_free_recentering_rejects_key_overflow_before_symbolica_translation() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "split-affine-key-overflow", 1).unwrap();
        let mut source = ParametricRelation::new(
            "family",
            ParametricRowId::Derived {
                label: Arc::from("split-affine-overflow-source"),
            },
            &context,
        );
        source
            .add_term(
                &context,
                IndexShift::try_new([i64::MIN], 1).unwrap(),
                context.one(),
            )
            .unwrap();
        assert!(matches!(
            source.affine_free_recentered(
                &context,
                &IndexShift::try_new([0], 1).unwrap(),
                &IndexShift::try_new([1], 1).unwrap(),
                ParametricRowId::Derived {
                    label: Arc::from("split-affine-overflow-target")
                },
                ParametricAffineFreeRecenteringLimits::default(),
            ),
            Err(ParametricRelationError::IndexOverflow { position: 0 })
        ));
    }
}
