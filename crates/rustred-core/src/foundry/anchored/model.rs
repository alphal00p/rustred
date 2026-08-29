use std::sync::Arc;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralKey;
use crate::identity::{IdentityConditionSource, RowId};
use crate::sector::{OrderingPolicy, StrictDescentWitness};

/// One strictly lower term on the right-hand side of an anchored rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredRuleTerm {
    integral: IntegralKey,
    coefficient: Coefficient,
    descent: StrictDescentWitness,
}

impl AnchoredRuleTerm {
    pub fn integral(&self) -> &IntegralKey {
        &self.integral
    }

    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn descent(&self) -> &StrictDescentWitness {
        &self.descent
    }
}

/// A chronological source-row weight in the exact replay combination.
///
/// Entries are retained in increasing source ordinal. An omitted ordinal has
/// exact zero weight.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRowContribution {
    source_ordinal: usize,
    row_id: RowId,
    coefficient: Coefficient,
}

impl SourceRowContribution {
    pub fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
}

/// Why one concrete base-field polynomial must remain nonzero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardOrigin {
    SourceCondition {
        source_ordinal: usize,
        row_id: RowId,
        condition_ordinal: usize,
        condition_sources: Box<[IdentityConditionSource]>,
    },
    SourceCoefficientDenominator {
        source_ordinal: usize,
        row_id: RowId,
        shift: Box<[i64]>,
    },
    ReducerPivotNumerator {
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
    },
    ReducerPivotDenominator {
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
    },
    RuleCoefficientDenominator {
        integral: IntegralKey,
    },
    SourceCombinationDenominator {
        source_ordinal: usize,
        row_id: RowId,
    },
}

/// One deduplicated, nonconstant polynomial guard with all retained origins.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredNonZeroGuard {
    pub(super) polynomial: CoefficientPolynomial,
    pub(super) origins: Vec<GuardOrigin>,
}

impl AnchoredNonZeroGuard {
    pub fn polynomial(&self) -> &CoefficientPolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &[GuardOrigin] {
        &self.origins
    }
}

/// One exact coefficient inverted along the native reducer's chosen path.
///
/// Its numerator is the pivot's nonzero condition before normalization. The
/// value is retained even when that numerator is a nonzero constant and thus
/// does not need an entry in [`AnchoredRule::nonzero_guards`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReducerPivotGuard {
    source_ordinal: usize,
    row_id: RowId,
    pivot_column: usize,
    coefficient: Coefficient,
    nonzero_polynomial: CoefficientPolynomial,
}

impl ReducerPivotGuard {
    pub fn source_ordinal(&self) -> usize {
        self.source_ordinal
    }

    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }

    pub fn pivot_column(&self) -> usize {
        self.pivot_column
    }

    pub fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn nonzero_polynomial(&self) -> &CoefficientPolynomial {
        &self.nonzero_polynomial
    }
}

/// Counts fixed by a successful exact source-row replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactReplayWitness {
    source_rows_used: usize,
    integral_columns_checked: usize,
    exact_operations: usize,
}

impl ExactReplayWitness {
    pub fn source_rows_used(self) -> usize {
        self.source_rows_used
    }

    pub fn integral_columns_checked(self) -> usize {
        self.integral_columns_checked
    }

    pub fn exact_operations(self) -> usize {
        self.exact_operations
    }
}

/// One concrete, guarded, exactly replayed replacement rule.
///
/// This value certifies only its exact anchor and source-row span. It carries
/// no claim of exceptional-domain coverage, sector closure, or artifact
/// publication.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchoredRule {
    pub(super) family_fingerprint: Arc<String>,
    pub(super) anchor: IntegralKey,
    pub(super) ordering: OrderingPolicy,
    pub(super) pivot: IntegralKey,
    pub(super) right_hand_side: Vec<AnchoredRuleTerm>,
    pub(super) pivot_guards: Vec<ReducerPivotGuard>,
    pub(super) nonzero_guards: Vec<AnchoredNonZeroGuard>,
    pub(super) source_combination: Vec<SourceRowContribution>,
    pub(super) replay: ExactReplayWitness,
}

impl AnchoredRule {
    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    /// The exact integer assignment at which source coefficients were
    /// specialized. This is the rule's integer-domain guard.
    pub fn anchor(&self) -> &IntegralKey {
        &self.anchor
    }

    pub fn ordering(&self) -> OrderingPolicy {
        self.ordering
    }

    pub fn pivot(&self) -> &IntegralKey {
        &self.pivot
    }

    pub fn right_hand_side(&self) -> &[AnchoredRuleTerm] {
        &self.right_hand_side
    }

    pub fn pivot_guard(&self) -> &ReducerPivotGuard {
        self.pivot_guards
            .last()
            .expect("an anchored rule always retains its chosen pivot")
    }

    /// Every pre-normalization pivot used along the chosen reducer row's
    /// chronological elimination path. The chosen rule pivot is last.
    pub fn elimination_pivot_guards(&self) -> &[ReducerPivotGuard] {
        &self.pivot_guards
    }

    pub fn nonzero_guards(&self) -> &[AnchoredNonZeroGuard] {
        &self.nonzero_guards
    }

    pub fn source_combination(&self) -> &[SourceRowContribution] {
        &self.source_combination
    }

    pub fn replay(&self) -> ExactReplayWitness {
        self.replay
    }
}

impl AnchoredRuleTerm {
    pub(super) fn new(
        integral: IntegralKey,
        coefficient: Coefficient,
        descent: StrictDescentWitness,
    ) -> Self {
        Self {
            integral,
            coefficient,
            descent,
        }
    }
}

impl SourceRowContribution {
    pub(super) fn new(source_ordinal: usize, row_id: RowId, coefficient: Coefficient) -> Self {
        Self {
            source_ordinal,
            row_id,
            coefficient,
        }
    }
}

impl ReducerPivotGuard {
    pub(super) fn new(
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
        coefficient: Coefficient,
    ) -> Self {
        let nonzero_polynomial = coefficient.numerator.clone();
        Self {
            source_ordinal,
            row_id,
            pivot_column,
            coefficient,
            nonzero_polynomial,
        }
    }
}

impl ExactReplayWitness {
    pub(super) fn new(
        source_rows_used: usize,
        integral_columns_checked: usize,
        exact_operations: usize,
    ) -> Self {
        Self {
            source_rows_used,
            integral_columns_checked,
            exact_operations,
        }
    }
}
