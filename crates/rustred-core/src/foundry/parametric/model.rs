use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::foundry::anchored::AnchoredRule;
use crate::identity::{IdentityConditionSource, IndexShift, RowId};
use crate::sector::{Mask, OrderingPolicy, SectorInteriorDomain, ShiftStrictDescentWitness};

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

/// Independent concrete evidence that the parametric rule specializes to the
/// existing anchored elimination result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorAgreement {
    anchored_rule: AnchoredRule,
    specialized_right_hand_side_terms: usize,
    specialized_source_terms: usize,
    nonzero_guards_checked: usize,
}

impl AnchorAgreement {
    pub fn anchored_rule(&self) -> &AnchoredRule {
        &self.anchored_rule
    }

    pub fn specialized_right_hand_side_terms(&self) -> usize {
        self.specialized_right_hand_side_terms
    }

    pub fn specialized_source_terms(&self) -> usize {
        self.specialized_source_terms
    }

    pub fn nonzero_guards_checked(&self) -> usize {
        self.nonzero_guards_checked
    }
}

/// One guarded, uniformly descending, exactly replayed parametric rule.
///
/// This object certifies only its fixed-sector interior, supplied source-row
/// span, and concrete anchor agreement. It does not certify exceptional
/// branches, dependency closure, or a published reduction artifact.
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
    pub(super) anchor_agreement: AnchorAgreement,
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

    pub fn anchor_agreement(&self) -> &AnchorAgreement {
        &self.anchor_agreement
    }

    pub fn anchor(&self) -> &IntegralKey {
        self.anchor_agreement.anchored_rule().anchor()
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

impl AnchorAgreement {
    pub(super) fn new(
        anchored_rule: AnchoredRule,
        specialized_right_hand_side_terms: usize,
        specialized_source_terms: usize,
        nonzero_guards_checked: usize,
    ) -> Self {
        Self {
            anchored_rule,
            specialized_right_hand_side_terms,
            specialized_source_terms,
            nonzero_guards_checked,
        }
    }
}
