use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::identity::{IdentityConditionSource, IndexShift, RowId};
use crate::sector::{Mask, OrderingPolicy, SectorInteriorDomain, ShiftStrictDescentWitness};

use super::boundary::SectorMonotoneTargetAdmission;

/// One uniformly lower shift on the right-hand side of a parametric rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricRuleTerm {
    shift: IndexShift,
    coefficient: IndexedCoefficient,
    descent: ShiftStrictDescentWitness,
}

impl ParametricRuleTerm {
    /// Integral displacement relative to the rule's free index vector.
    pub fn shift(&self) -> &IndexShift {
        &self.shift
    }

    /// Exact coefficient in the authenticated indexed field `K(n)`.
    pub fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    /// Structural proof that this shift is below the pivot everywhere in the
    /// returned fixed-sector interior.
    pub fn descent(&self) -> &ShiftStrictDescentWitness {
        &self.descent
    }
}

/// A chronological source-row weight in the exact indexed replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricSourceRowContribution {
    source_ordinal: usize,
    row_id: RowId,
    coefficient: IndexedCoefficient,
}

impl ParametricSourceRowContribution {
    pub fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }
}

/// Why one polynomial over `K[n]` must remain nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricGuardOrigin {
    SourceCondition {
        source_ordinal: usize,
        row_id: RowId,
        condition_ordinal: usize,
        condition_sources: Box<[IdentityConditionSource]>,
    },
    SourceCoefficientDenominator {
        source_ordinal: usize,
        row_id: RowId,
        shift: IndexShift,
    },
    ReducerPivotNumerator {
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
        pivot_shift: IndexShift,
    },
    ReducerPivotDenominator {
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
        pivot_shift: IndexShift,
    },
    RuleCoefficientDenominator {
        shift: IndexShift,
    },
    SourceCombinationDenominator {
        source_ordinal: usize,
        row_id: RowId,
    },
}

/// One deduplicated parametric nonzero guard with complete retained origins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroGuard {
    pub(super) polynomial: IndexedPolynomial,
    pub(super) origins: Vec<ParametricGuardOrigin>,
}

impl ParametricNonZeroGuard {
    pub fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &[ParametricGuardOrigin] {
        &self.origins
    }
}

/// One exact pre-normalization coefficient inverted on the selected native
/// reducer path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricReducerPivotGuard {
    source_ordinal: usize,
    row_id: RowId,
    pivot_column: usize,
    pivot_shift: IndexShift,
    coefficient: IndexedCoefficient,
    nonzero_polynomial: IndexedPolynomial,
}

impl ParametricReducerPivotGuard {
    pub fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn pivot_column(&self) -> usize {
        self.pivot_column
    }

    pub fn pivot_shift(&self) -> &IndexShift {
        &self.pivot_shift
    }

    pub fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    pub fn nonzero_polynomial(&self) -> &IndexedPolynomial {
        &self.nonzero_polynomial
    }
}

/// Counts fixed by a successful exact indexed source-row replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricExactReplayWitness {
    source_rows_used: usize,
    shift_columns_checked: usize,
    exact_operations: usize,
}

impl ParametricExactReplayWitness {
    pub fn source_rows_used(self) -> usize {
        self.source_rows_used
    }

    pub fn shift_columns_checked(self) -> usize {
        self.shift_columns_checked
    }

    pub fn exact_operations(self) -> usize {
        self.exact_operations
    }
}

/// Exact base-field replay of the retained indexed source combination at one
/// concrete lattice point.
///
/// The witness authenticates the rule relation itself. It deliberately does
/// not compare against another elimination normal form: a boundary sector can
/// induce a different concrete column ordering even though both rows belong
/// to the same exact source span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConcreteSpecializationReplayWitness {
    anchor: IntegralKey,
    source_contributions_checked: usize,
    source_terms_checked: usize,
    right_hand_side_terms_checked: usize,
    integral_keys_checked: usize,
    nonzero_guards_checked: usize,
    exact_operations: usize,
    peak_retained_coefficient_terms: usize,
}

impl ConcreteSpecializationReplayWitness {
    pub fn anchor(&self) -> &IntegralKey {
        &self.anchor
    }

    pub fn source_contributions_checked(&self) -> usize {
        self.source_contributions_checked
    }

    pub fn source_terms_checked(&self) -> usize {
        self.source_terms_checked
    }

    pub fn right_hand_side_terms_checked(&self) -> usize {
        self.right_hand_side_terms_checked
    }

    /// Number of source-term, pivot, and RHS keys constructed and checked.
    /// Zero-specialized terms remain included in this deterministic count.
    pub fn integral_keys_checked(&self) -> usize {
        self.integral_keys_checked
    }

    pub fn nonzero_guards_checked(&self) -> usize {
        self.nonzero_guards_checked
    }

    pub fn exact_operations(&self) -> usize {
        self.exact_operations
    }

    /// Peak aggregate numerator-plus-denominator term count retained by the
    /// exact base-field accumulator after any deterministic map transition.
    pub fn peak_retained_coefficient_terms(&self) -> usize {
        self.peak_retained_coefficient_terms
    }
}

/// One guarded, uniformly descending, exactly replayed parametric rule.
///
/// This object certifies only its fixed-sector interior, supplied source-row
/// span, and concrete specialization replay. For a sector-monotone target
/// derivation, the replay anchor may lie outside that same-sector interior;
/// [`Self::sector_monotone_admission`] then supplies a larger universal parent
/// box and exhaustive term-local pinch proofs. It does not certify exceptional
/// guards, lower-sector rule availability, dependency closure, or a published
/// reduction artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricRule {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) domain: SectorInteriorDomain,
    pub(super) ordering: OrderingPolicy,
    pub(super) pivot: IndexShift,
    pub(super) right_hand_side: Vec<ParametricRuleTerm>,
    pub(super) pivot_guards: Vec<ParametricReducerPivotGuard>,
    pub(super) nonzero_guards: Vec<ParametricNonZeroGuard>,
    pub(super) source_combination: Vec<ParametricSourceRowContribution>,
    pub(super) replay: ParametricExactReplayWitness,
    pub(super) concrete_replay: ConcreteSpecializationReplayWitness,
    pub(super) sector_monotone_admission: Option<SectorMonotoneTargetAdmission>,
}

impl ParametricRule {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    pub fn sector(&self) -> &Mask {
        self.domain.sector()
    }

    pub fn domain(&self) -> &SectorInteriorDomain {
        &self.domain
    }

    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub fn pivot(&self) -> &IndexShift {
        &self.pivot
    }

    pub fn right_hand_side(&self) -> &[ParametricRuleTerm] {
        &self.right_hand_side
    }

    pub fn pivot_guard(&self) -> &ParametricReducerPivotGuard {
        self.pivot_guards
            .last()
            .expect("a parametric rule always retains its chosen pivot")
    }

    pub fn elimination_pivot_guards(&self) -> &[ParametricReducerPivotGuard] {
        &self.pivot_guards
    }

    pub fn nonzero_guards(&self) -> &[ParametricNonZeroGuard] {
        &self.nonzero_guards
    }

    pub fn source_combination(&self) -> &[ParametricSourceRowContribution] {
        &self.source_combination
    }

    pub fn replay(&self) -> ParametricExactReplayWitness {
        self.replay
    }

    pub fn concrete_replay(&self) -> &ConcreteSpecializationReplayWitness {
        &self.concrete_replay
    }

    /// Universal parent-box and term-local pinch evidence produced only by the
    /// sector-monotone target API. Interior-only derivations return `None`.
    pub fn sector_monotone_admission(&self) -> Option<&SectorMonotoneTargetAdmission> {
        self.sector_monotone_admission.as_ref()
    }

    pub fn anchor(&self) -> &IntegralKey {
        self.concrete_replay.anchor()
    }
}

impl ParametricRuleTerm {
    pub(super) fn new(
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        descent: ShiftStrictDescentWitness,
    ) -> Self {
        Self {
            shift,
            coefficient,
            descent,
        }
    }
}

impl ParametricSourceRowContribution {
    pub(super) fn new(
        source_ordinal: usize,
        row_id: RowId,
        coefficient: IndexedCoefficient,
    ) -> Self {
        Self {
            source_ordinal,
            row_id,
            coefficient,
        }
    }
}

impl ParametricReducerPivotGuard {
    pub(super) fn new(
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
        pivot_shift: IndexShift,
        coefficient: IndexedCoefficient,
        nonzero_polynomial: IndexedPolynomial,
    ) -> Self {
        Self {
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
            coefficient,
            nonzero_polynomial,
        }
    }
}

impl ParametricExactReplayWitness {
    pub(super) fn new(
        source_rows_used: usize,
        shift_columns_checked: usize,
        exact_operations: usize,
    ) -> Self {
        Self {
            source_rows_used,
            shift_columns_checked,
            exact_operations,
        }
    }
}

impl ConcreteSpecializationReplayWitness {
    pub(super) fn new(
        anchor: IntegralKey,
        source_contributions_checked: usize,
        source_terms_checked: usize,
        right_hand_side_terms_checked: usize,
        integral_keys_checked: usize,
        nonzero_guards_checked: usize,
        exact_operations: usize,
        peak_retained_coefficient_terms: usize,
    ) -> Self {
        Self {
            anchor,
            source_contributions_checked,
            source_terms_checked,
            right_hand_side_terms_checked,
            integral_keys_checked,
            nonzero_guards_checked,
            exact_operations,
            peak_retained_coefficient_terms,
        }
    }
}
