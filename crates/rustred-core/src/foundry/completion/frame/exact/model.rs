use std::sync::Arc;

use crate::algebra::{IndexedCoefficient, IndexedPolynomial};
use crate::foundry::completion::stratum::{
    DecoratedStratumId, ImmutableOwnerSnapshotId, ProperSubsectorOwner,
};
use crate::identity::{IdentityConditionSource, IntegralShift};
use crate::sector::SectorMonotoneShiftDescentWitness;

use super::super::SourceInstanceId;
use super::super::modular::{ModularRankDiagnostics, ModularSampleFingerprint};
use super::super::{PhysicalFramePlan, PhysicalFramePlanIdentity};

/// One allowed physical residual in the normalized exact zero equation
/// `target + sum(coefficient * integral) = 0`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitTerm {
    physical_column: usize,
    shift: IntegralShift,
    coefficient: IndexedCoefficient,
    descent: SectorMonotoneShiftDescentWitness,
    proper_subsector_owners: Box<[ProperSubsectorOwner]>,
}

impl ExactCircuitTerm {
    pub(crate) const fn physical_column(&self) -> usize {
        self.physical_column
    }

    pub(crate) const fn shift(&self) -> &IntegralShift {
        &self.shift
    }

    pub(crate) const fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    pub(crate) const fn descent(&self) -> &SectorMonotoneShiftDescentWitness {
        &self.descent
    }

    pub(crate) fn proper_subsector_owners(&self) -> &[ProperSubsectorOwner] {
        &self.proper_subsector_owners
    }

    pub(super) fn new(
        physical_column: usize,
        shift: IntegralShift,
        coefficient: IndexedCoefficient,
        descent: SectorMonotoneShiftDescentWitness,
        proper_subsector_owners: Vec<ProperSubsectorOwner>,
    ) -> Self {
        Self {
            physical_column,
            shift,
            coefficient,
            descent,
            proper_subsector_owners: proper_subsector_owners.into_boxed_slice(),
        }
    }
}

/// One original translated source row in the exact normalized circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactFrameSourceContribution {
    frame_row_ordinal: usize,
    source_instance: SourceInstanceId,
    coefficient: IndexedCoefficient,
}

impl ExactFrameSourceContribution {
    pub(crate) const fn frame_row_ordinal(&self) -> usize {
        self.frame_row_ordinal
    }

    pub(crate) const fn source_instance(&self) -> &SourceInstanceId {
        &self.source_instance
    }

    pub(crate) const fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    pub(super) fn new(
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        coefficient: IndexedCoefficient,
    ) -> Self {
        Self {
            frame_row_ordinal,
            source_instance,
            coefficient,
        }
    }
}

/// Why one exact polynomial must remain nonzero for this circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitGuardOrigin {
    SourceCondition {
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        condition_ordinal: usize,
        condition_sources: Box<[IdentityConditionSource]>,
    },
    SourceCoefficientDenominator {
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        physical_column: usize,
    },
    ReducerPivotNumerator {
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        physical_pivot_column: usize,
    },
    ReducerPivotDenominator {
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        physical_pivot_column: usize,
    },
    SourceMultiplierDenominator {
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
    },
    ResidualCoefficientDenominator {
        physical_column: usize,
    },
}

/// One exact nonzero guard with deterministic complete origin chronology.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitGuard {
    polynomial: IndexedPolynomial,
    origins: Box<[ExactCircuitGuardOrigin]>,
}

impl ExactCircuitGuard {
    pub(crate) const fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }

    pub(crate) fn origins(&self) -> &[ExactCircuitGuardOrigin] {
        &self.origins
    }

    pub(super) fn new(
        polynomial: IndexedPolynomial,
        origins: Vec<ExactCircuitGuardOrigin>,
    ) -> Self {
        Self {
            polynomial,
            origins: origins.into_boxed_slice(),
        }
    }
}

/// One pre-normalization exact pivot inverted by the forward reducer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitPivotGuard {
    frame_row_ordinal: usize,
    source_instance: SourceInstanceId,
    physical_pivot_column: usize,
    coefficient: IndexedCoefficient,
    nonzero_polynomial: IndexedPolynomial,
}

impl ExactCircuitPivotGuard {
    pub(crate) const fn frame_row_ordinal(&self) -> usize {
        self.frame_row_ordinal
    }

    pub(crate) const fn source_instance(&self) -> &SourceInstanceId {
        &self.source_instance
    }

    pub(crate) const fn physical_pivot_column(&self) -> usize {
        self.physical_pivot_column
    }

    pub(crate) const fn coefficient(&self) -> &IndexedCoefficient {
        &self.coefficient
    }

    pub(crate) const fn nonzero_polynomial(&self) -> &IndexedPolynomial {
        &self.nonzero_polynomial
    }

    pub(super) fn new(
        frame_row_ordinal: usize,
        source_instance: SourceInstanceId,
        physical_pivot_column: usize,
        coefficient: IndexedCoefficient,
        nonzero_polynomial: IndexedPolynomial,
    ) -> Self {
        Self {
            frame_row_ordinal,
            source_instance,
            physical_pivot_column,
            coefficient,
            nonzero_polynomial,
        }
    }
}

/// Deterministic counts from the independent full-column exact replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitReplayWitness {
    source_contributions: usize,
    source_terms: usize,
    physical_columns: usize,
    exact_operations: usize,
}

impl ExactCircuitReplayWitness {
    pub(crate) const fn source_contributions(self) -> usize {
        self.source_contributions
    }

    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn physical_columns(self) -> usize {
        self.physical_columns
    }

    pub(crate) const fn exact_operations(self) -> usize {
        self.exact_operations
    }

    pub(super) const fn new(
        source_contributions: usize,
        source_terms: usize,
        physical_columns: usize,
        exact_operations: usize,
    ) -> Self {
        Self {
            source_contributions,
            source_terms,
            physical_columns,
            exact_operations,
        }
    }
}

/// Exactly replayed source-span circuit discovered by one modular sample.
///
/// This value proves its guarded algebraic relation only. The caller-supplied
/// column partition is not promoted into completion or stratum authority.
#[derive(Clone, Debug)]
pub(crate) struct ExactTargetCircuit {
    plan_identity: PhysicalFramePlanIdentity,
    sample: Arc<ModularSampleFingerprint>,
    stratum_id: DecoratedStratumId,
    owner_snapshot_id: ImmutableOwnerSnapshotId,
    modular_diagnostics: ModularRankDiagnostics,
    target_column: usize,
    target_shift: IntegralShift,
    residual_terms: Box<[ExactCircuitTerm]>,
    source_combination: Box<[ExactFrameSourceContribution]>,
    pivot_guards: Box<[ExactCircuitPivotGuard]>,
    nonzero_guards: Box<[ExactCircuitGuard]>,
    replay: ExactCircuitReplayWitness,
}

// The live-plan token is an in-memory admission seal, not mathematical
// payload. Structural equality is used by deterministic campaign regressions
// and therefore deliberately excludes `plan_identity`; authority checks use
// `is_bound_to` instead.
impl PartialEq for ExactTargetCircuit {
    fn eq(&self, other: &Self) -> bool {
        self.sample == other.sample
            && self.stratum_id == other.stratum_id
            && self.owner_snapshot_id == other.owner_snapshot_id
            && self.modular_diagnostics == other.modular_diagnostics
            && self.target_column == other.target_column
            && self.target_shift == other.target_shift
            && self.residual_terms == other.residual_terms
            && self.source_combination == other.source_combination
            && self.pivot_guards == other.pivot_guards
            && self.nonzero_guards == other.nonzero_guards
            && self.replay == other.replay
    }
}

impl Eq for ExactTargetCircuit {}

impl ExactTargetCircuit {
    pub(crate) fn is_bound_to(&self, plan: &PhysicalFramePlan) -> bool {
        self.plan_identity.belongs_to(plan)
    }

    #[cfg(test)]
    pub(crate) fn replace_first_guard_polynomial_for_test(
        &mut self,
        polynomial: IndexedPolynomial,
    ) {
        self.nonzero_guards[0].polynomial = polynomial;
    }

    pub(crate) fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn stratum_id(&self) -> &DecoratedStratumId {
        &self.stratum_id
    }

    pub(crate) const fn owner_snapshot_id(&self) -> &ImmutableOwnerSnapshotId {
        &self.owner_snapshot_id
    }

    pub(crate) const fn modular_diagnostics(&self) -> &ModularRankDiagnostics {
        &self.modular_diagnostics
    }

    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(crate) const fn target_shift(&self) -> &IntegralShift {
        &self.target_shift
    }

    pub(crate) fn residual_terms(&self) -> &[ExactCircuitTerm] {
        &self.residual_terms
    }

    pub(crate) fn source_combination(&self) -> &[ExactFrameSourceContribution] {
        &self.source_combination
    }

    pub(crate) fn pivot_guards(&self) -> &[ExactCircuitPivotGuard] {
        &self.pivot_guards
    }

    pub(crate) fn nonzero_guards(&self) -> &[ExactCircuitGuard] {
        &self.nonzero_guards
    }

    pub(crate) const fn replay(&self) -> ExactCircuitReplayWitness {
        self.replay
    }

    #[cfg(test)]
    pub(crate) fn replace_first_source_coefficient_for_test(
        &mut self,
        coefficient: IndexedCoefficient,
    ) {
        self.source_combination[0].coefficient = coefficient;
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        plan_identity: PhysicalFramePlanIdentity,
        sample: Arc<ModularSampleFingerprint>,
        stratum_id: DecoratedStratumId,
        owner_snapshot_id: ImmutableOwnerSnapshotId,
        modular_diagnostics: ModularRankDiagnostics,
        target_column: usize,
        target_shift: IntegralShift,
        residual_terms: Vec<ExactCircuitTerm>,
        source_combination: Vec<ExactFrameSourceContribution>,
        pivot_guards: Vec<ExactCircuitPivotGuard>,
        nonzero_guards: Vec<ExactCircuitGuard>,
        replay: ExactCircuitReplayWitness,
    ) -> Self {
        Self {
            plan_identity,
            sample,
            stratum_id,
            owner_snapshot_id,
            modular_diagnostics,
            target_column,
            target_shift,
            residual_terms: residual_terms.into_boxed_slice(),
            source_combination: source_combination.into_boxed_slice(),
            pivot_guards: pivot_guards.into_boxed_slice(),
            nonzero_guards: nonzero_guards.into_boxed_slice(),
            replay,
        }
    }
}

/// Typed inconclusive result when a modularly selected support does not retain
/// an exact target pivot. It is never evidence for no relation in the full
/// declared frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExactCircuitSupportDidNotLift {
    sample: Arc<ModularSampleFingerprint>,
    modular_diagnostics: ModularRankDiagnostics,
    selected_source_instances: Box<[SourceInstanceId]>,
    exact_forbidden_rank: usize,
    exact_augmented_rank: usize,
}

impl ExactCircuitSupportDidNotLift {
    pub(crate) fn sample_fingerprint(&self) -> &Arc<ModularSampleFingerprint> {
        &self.sample
    }

    pub(crate) const fn modular_diagnostics(&self) -> &ModularRankDiagnostics {
        &self.modular_diagnostics
    }

    pub(crate) fn selected_source_instances(&self) -> &[SourceInstanceId] {
        &self.selected_source_instances
    }

    pub(crate) const fn exact_forbidden_rank(&self) -> usize {
        self.exact_forbidden_rank
    }

    pub(crate) const fn exact_augmented_rank(&self) -> usize {
        self.exact_augmented_rank
    }

    pub(super) fn new(
        sample: Arc<ModularSampleFingerprint>,
        modular_diagnostics: ModularRankDiagnostics,
        selected_source_instances: Vec<SourceInstanceId>,
        exact_forbidden_rank: usize,
        exact_augmented_rank: usize,
    ) -> Self {
        Self {
            sample,
            modular_diagnostics,
            selected_source_instances: selected_source_instances.into_boxed_slice(),
            exact_forbidden_rank,
            exact_augmented_rank,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactCircuitLift {
    Replayed(ExactTargetCircuit),
    ModularSupportDidNotLift(ExactCircuitSupportDidNotLift),
}
