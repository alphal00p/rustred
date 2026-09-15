use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::family::IntegralKey;
use crate::foundry::completion::frame::exact::ExactCircuitLoweringSeal;
use crate::identity::{IdentityConditionSource, IndexShift, RowId};
use crate::sector::{
    Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneShiftDescentWitness,
    ShiftStrictDescentWitness,
};

use super::boundary::SectorMonotoneTargetAdmission;
use super::evidence::ParametricReplayEvidence;

/// One uniformly lower shift on the right-hand side of a parametric rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricRuleTerm {
    shift: IndexShift,
    coefficient: IndexedCoefficient,
    descent: ParametricRuleTermDescent,
}

/// The exact ordering authority carried by one RHS term.
///
/// Ordinary identities retain a uniform same-sector witness.  A rule proved
/// only on a boundary face/ray instead retains the stronger piecewise witness
/// that classifies every point as same-sector descent or proper-subsector
/// descent.  The latter is required for terms that are always pinched on the
/// declared quotient and therefore have no meaningful same-sector cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricRuleTermDescent {
    FixedSector(ShiftStrictDescentWitness),
    SectorMonotone(SectorMonotoneShiftDescentWitness),
}

impl ParametricRuleTermDescent {
    pub fn verify(&self) -> bool {
        match self {
            Self::FixedSector(witness) => witness.verify(),
            Self::SectorMonotone(witness) => witness.verify(),
        }
    }

    pub fn fixed_sector(&self) -> Option<&ShiftStrictDescentWitness> {
        match self {
            Self::FixedSector(witness) => Some(witness),
            Self::SectorMonotone(_) => None,
        }
    }

    pub fn sector_monotone(&self) -> Option<&SectorMonotoneShiftDescentWitness> {
        match self {
            Self::FixedSector(_) => None,
            Self::SectorMonotone(witness) => Some(witness),
        }
    }
}

impl From<ShiftStrictDescentWitness> for ParametricRuleTermDescent {
    fn from(value: ShiftStrictDescentWitness) -> Self {
        Self::FixedSector(value)
    }
}

impl From<SectorMonotoneShiftDescentWitness> for ParametricRuleTermDescent {
    fn from(value: SectorMonotoneShiftDescentWitness) -> Self {
        Self::SectorMonotone(value)
    }
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

    /// Structural proof that this shift is below the pivot throughout the
    /// rule's ordinary interior or piecewise parent-sector domain.
    pub fn descent(&self) -> &ParametricRuleTermDescent {
        &self.descent
    }
}

impl ParametricNonZeroGuard {
    pub(crate) fn from_replayed_exact_parts(
        _seal: &ExactCircuitLoweringSeal,
        polynomial: IndexedPolynomial,
        origins: Vec<ParametricGuardOrigin>,
    ) -> Self {
        Self {
            polynomial,
            origins,
        }
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
    /// The coefficient of the target in a fully replayed fraction-free
    /// ordinary-source consequence.
    FinalTargetCoefficient,
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
/// The replay evidence identifies its proof convention. Anchored derivations
/// certify a supplied source-row span and a concrete specialization; their
/// anchor may lie outside the fixed-sector interior when
/// [`Self::sector_monotone_admission`] supplies a larger parent box and exact
/// pinch proofs. Combined-original-domain evidence instead describes full
/// weighted source replay on its retained coordinate domain, without an
/// elimination anchor or pivot history. Its cold producer is not yet enabled.
/// Neither convention alone certifies exceptional-guard coverage, lower-sector
/// availability, dependency closure, or a published reduction artifact.
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
    pub(super) replay_evidence: ParametricReplayEvidence,
    pub(super) sector_monotone_admission: Option<SectorMonotoneTargetAdmission>,
}

impl ParametricRule {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_replayed_exact_parts(
        _seal: &ExactCircuitLoweringSeal,
        family_fingerprint: Arc<String>,
        context_fingerprint: Arc<String>,
        domain: SectorInteriorDomain,
        ordering: OrderingPolicy,
        pivot: IndexShift,
        right_hand_side: Vec<ParametricRuleTerm>,
        pivot_guards: Vec<ParametricReducerPivotGuard>,
        nonzero_guards: Vec<ParametricNonZeroGuard>,
        source_combination: Vec<ParametricSourceRowContribution>,
        replay: ParametricExactReplayWitness,
        concrete_replay: ConcreteSpecializationReplayWitness,
        sector_monotone_admission: SectorMonotoneTargetAdmission,
    ) -> Self {
        Self {
            family_fingerprint,
            context_fingerprint,
            domain,
            ordering,
            pivot,
            right_hand_side,
            pivot_guards,
            nonzero_guards,
            source_combination,
            replay_evidence: ParametricReplayEvidence::Anchored {
                indexed: replay,
                concrete: concrete_replay,
            },
            sector_monotone_admission: Some(sector_monotone_admission),
        }
    }

    pub fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

    #[cfg(test)]
    pub(crate) fn replace_first_guard_polynomial_for_test(
        &mut self,
        polynomial: IndexedPolynomial,
    ) {
        self.nonzero_guards[0].polynomial = polynomial;
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

    /// Final elimination pivot, when this producer performed elimination.
    pub fn pivot_guard(&self) -> Option<&ParametricReducerPivotGuard> {
        self.elimination_pivot_guards().last()
    }

    pub fn elimination_pivot_guards(&self) -> &[ParametricReducerPivotGuard] {
        match &self.replay_evidence {
            ParametricReplayEvidence::Anchored { .. } => &self.pivot_guards,
            ParametricReplayEvidence::CombinedOriginalDomain(_) => &[],
        }
    }

    pub fn nonzero_guards(&self) -> &[ParametricNonZeroGuard] {
        &self.nonzero_guards
    }

    pub fn source_combination(&self) -> &[ParametricSourceRowContribution] {
        &self.source_combination
    }

    pub fn replay_evidence(&self) -> &ParametricReplayEvidence {
        &self.replay_evidence
    }

    /// Indexed elimination replay, absent for a combined-domain identity.
    pub fn replay(&self) -> Option<ParametricExactReplayWitness> {
        self.replay_evidence.indexed()
    }

    /// Concrete specialization evidence, when supplied by this producer.
    pub fn concrete_replay(&self) -> Option<&ConcreteSpecializationReplayWitness> {
        self.replay_evidence.concrete()
    }

    /// Universal parent-box and term-local pinch evidence produced only by the
    /// sector-monotone target API. Interior-only derivations return `None`.
    pub fn sector_monotone_admission(&self) -> Option<&SectorMonotoneTargetAdmission> {
        self.sector_monotone_admission.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn replace_ordering_for_artifact_test(&mut self, ordering: OrderingPolicy) {
        self.ordering = ordering;
    }

    #[cfg(test)]
    pub(crate) fn replace_domain_for_artifact_test(&mut self, domain: SectorInteriorDomain) {
        self.domain = domain;
    }

    #[cfg(test)]
    pub(crate) fn replace_rhs_descent_with_admission_for_artifact_test(&mut self) {
        let admission = self
            .sector_monotone_admission
            .as_ref()
            .expect("test rule must own a sector-monotone admission");
        assert_eq!(self.right_hand_side.len(), admission.dependencies().len());
        for (term, dependency) in self
            .right_hand_side
            .iter_mut()
            .zip(admission.dependencies())
        {
            term.descent = ParametricRuleTermDescent::SectorMonotone(dependency.descent().clone());
        }
    }

    pub fn anchor(&self) -> Option<&IntegralKey> {
        self.concrete_replay()
            .map(ConcreteSpecializationReplayWitness::anchor)
    }

    /// Deliberately unverified evidence mutation for cold-boundary rejection
    /// tests. This is not a producer and is absent from production builds.
    #[cfg(test)]
    pub(crate) fn replace_replay_with_uncertified_combined_domain_for_test(&mut self) {
        use super::evidence::CombinedOriginalDomainEvidence;
        use crate::foundry::completion::LatticeBox;

        let arity = self.sector().active_bits().len();
        let application = LatticeBox::try_new(vec![0; arity], vec![None; arity])
            .expect("test-only whole orthant");
        self.replay_evidence = ParametricReplayEvidence::CombinedOriginalDomain(Arc::new(
            CombinedOriginalDomainEvidence {
                sector: self.sector().clone(),
                fixed: Box::default(),
                application: vec![application].into(),
            },
        ));
    }

    #[cfg(test)]
    pub(crate) fn clear_nonzero_guards_for_test(&mut self) {
        self.nonzero_guards.clear();
    }
}

impl ParametricRuleTerm {
    pub(super) fn new(
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        descent: impl Into<ParametricRuleTermDescent>,
    ) -> Self {
        Self {
            shift,
            coefficient,
            descent: descent.into(),
        }
    }

    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        shift: IndexShift,
        coefficient: IndexedCoefficient,
        descent: SectorMonotoneShiftDescentWitness,
    ) -> Self {
        Self::new(shift, coefficient, descent)
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

    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        source_ordinal: usize,
        row_id: RowId,
        coefficient: IndexedCoefficient,
    ) -> Self {
        Self::new(source_ordinal, row_id, coefficient)
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

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        source_ordinal: usize,
        row_id: RowId,
        pivot_column: usize,
        pivot_shift: IndexShift,
        coefficient: IndexedCoefficient,
        nonzero_polynomial: IndexedPolynomial,
    ) -> Self {
        Self::new(
            source_ordinal,
            row_id,
            pivot_column,
            pivot_shift,
            coefficient,
            nonzero_polynomial,
        )
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

    pub(crate) fn from_exact_lowering(
        _seal: &ExactCircuitLoweringSeal,
        source_rows_used: usize,
        shift_columns_checked: usize,
        exact_operations: usize,
    ) -> Self {
        Self::new(source_rows_used, shift_columns_checked, exact_operations)
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
