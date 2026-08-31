use std::sync::Arc;

#[cfg(test)]
use crate::foundry::completion::frame::admission::compare_exact_circuit_content;
use crate::foundry::completion::frame::exact::{ExactCircuitSupportDidNotLift, ExactTargetCircuit};
use crate::foundry::completion::frame::modular::ModularRankDiagnostics;

use super::super::{CampaignError, CampaignModularProbe, FreshTaskEpoch};

/// Probe-local outcome after resampling and exact lifting on the common plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CanonicalRebaseAttemptOutcome {
    Replayed,
    NoModularHit { diagnostics: ModularRankDiagnostics },
    QueryRejected(CampaignError),
    SupportDidNotLift(ExactCircuitSupportDidNotLift),
}

/// Plan-free attempt telemetry in canonical raw-probe order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalRebaseAttempt {
    probe: CampaignModularProbe,
    outcome: CanonicalRebaseAttemptOutcome,
}

impl CanonicalRebaseAttempt {
    pub(crate) const fn probe(&self) -> &CampaignModularProbe {
        &self.probe
    }

    pub(crate) const fn outcome(&self) -> &CanonicalRebaseAttemptOutcome {
        &self.outcome
    }

    pub(super) const fn new(
        probe: CampaignModularProbe,
        outcome: CanonicalRebaseAttemptOutcome,
    ) -> Self {
        Self { probe, outcome }
    }
}

/// One unique exact proposal and its deterministic concrete replay anchor.
///
/// The circuit is bound to the containing batch's common epoch.  Supporting
/// probes are diagnostics only and grant no extra algebraic authority.
#[derive(Debug)]
pub(crate) struct CanonicalRebasedCandidate {
    circuit: Arc<ExactTargetCircuit>,
    anchor: Box<[i64]>,
    primary_probe: CampaignModularProbe,
    supporting_probes: Box<[CampaignModularProbe]>,
}

impl CanonicalRebasedCandidate {
    pub(crate) fn circuit(&self) -> &Arc<ExactTargetCircuit> {
        &self.circuit
    }

    pub(crate) fn anchor(&self) -> &[i64] {
        &self.anchor
    }

    pub(crate) const fn primary_probe(&self) -> &CampaignModularProbe {
        &self.primary_probe
    }

    pub(crate) fn supporting_probes(&self) -> &[CampaignModularProbe] {
        &self.supporting_probes
    }

    pub(super) fn new(
        circuit: Arc<ExactTargetCircuit>,
        anchor: Box<[i64]>,
        primary_probe: CampaignModularProbe,
        supporting_probes: Vec<CampaignModularProbe>,
    ) -> Self {
        Self {
            circuit,
            anchor,
            primary_probe,
            supporting_probes: supporting_probes.into_boxed_slice(),
        }
    }
}

/// Deterministic census for one canonical union/rebase transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalReplayTelemetry {
    replayed_nominations: usize,
    nomination_request_occurrences: usize,
    union_requests: usize,
    common_epoch_ordinal: usize,
    rebase_attempts: usize,
    successful_exact_lifts: usize,
    unique_candidates: usize,
    duplicate_exact_lifts: usize,
    anchor_coordinate_cells: usize,
    retained_diagnostic_entries: usize,
    retained_exact_payload_cells: usize,
    retained_integer_coefficient_bits: usize,
}

impl CanonicalReplayTelemetry {
    pub(crate) const fn replayed_nominations(self) -> usize {
        self.replayed_nominations
    }

    pub(crate) const fn nomination_request_occurrences(self) -> usize {
        self.nomination_request_occurrences
    }

    pub(crate) const fn union_requests(self) -> usize {
        self.union_requests
    }

    pub(crate) const fn common_epoch_ordinal(self) -> usize {
        self.common_epoch_ordinal
    }

    pub(crate) const fn rebase_attempts(self) -> usize {
        self.rebase_attempts
    }

    pub(crate) const fn successful_exact_lifts(self) -> usize {
        self.successful_exact_lifts
    }

    pub(crate) const fn unique_candidates(self) -> usize {
        self.unique_candidates
    }

    pub(crate) const fn duplicate_exact_lifts(self) -> usize {
        self.duplicate_exact_lifts
    }

    pub(crate) const fn anchor_coordinate_cells(self) -> usize {
        self.anchor_coordinate_cells
    }

    pub(crate) const fn retained_diagnostic_entries(self) -> usize {
        self.retained_diagnostic_entries
    }

    pub(crate) const fn retained_exact_payload_cells(self) -> usize {
        self.retained_exact_payload_cells
    }

    pub(crate) const fn retained_integer_coefficient_bits(self) -> usize {
        self.retained_integer_coefficient_bits
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new(
        replayed_nominations: usize,
        nomination_request_occurrences: usize,
        union_requests: usize,
        common_epoch_ordinal: usize,
        rebase_attempts: usize,
        successful_exact_lifts: usize,
        unique_candidates: usize,
        duplicate_exact_lifts: usize,
        anchor_coordinate_cells: usize,
        retained_diagnostic_entries: usize,
        retained_exact_payload_cells: usize,
        retained_integer_coefficient_bits: usize,
    ) -> Self {
        Self {
            replayed_nominations,
            nomination_request_occurrences,
            union_requests,
            common_epoch_ordinal,
            rebase_attempts,
            successful_exact_lifts,
            unique_candidates,
            duplicate_exact_lifts,
            anchor_coordinate_cells,
            retained_diagnostic_entries,
            retained_exact_payload_cells,
            retained_integer_coefficient_bits,
        }
    }
}

/// Common-plan exact proposals ready for guarded promotion.
#[derive(Debug)]
pub(crate) struct CanonicalReplayBatch {
    epoch: Arc<FreshTaskEpoch>,
    candidates: Box<[CanonicalRebasedCandidate]>,
    attempts: Box<[CanonicalRebaseAttempt]>,
    telemetry: CanonicalReplayTelemetry,
}

impl CanonicalReplayBatch {
    pub(crate) fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        &self.epoch
    }

    pub(crate) fn candidates(&self) -> &[CanonicalRebasedCandidate] {
        &self.candidates
    }

    pub(crate) fn attempts(&self) -> &[CanonicalRebaseAttempt] {
        &self.attempts
    }

    pub(crate) const fn telemetry(&self) -> CanonicalReplayTelemetry {
        self.telemetry
    }

    #[cfg(test)]
    pub(crate) fn replace_first_candidate_guard_polynomial_for_test(
        &mut self,
        polynomial: crate::algebra::IndexedPolynomial,
    ) {
        let candidate = self
            .candidates
            .first_mut()
            .expect("the test replay batch must contain a candidate");
        Arc::get_mut(&mut candidate.circuit)
            .expect("the test replay batch must uniquely own its exact circuit")
            .replace_first_guard_polynomial_for_test(polynomial);
    }

    #[cfg(test)]
    pub(crate) fn append_guard_modified_first_candidate_for_test(
        &mut self,
        polynomial: crate::algebra::IndexedPolynomial,
    ) -> Arc<ExactTargetCircuit> {
        let source = self
            .candidates
            .first()
            .expect("the test replay batch must contain a candidate");
        let mut circuit = source.circuit.as_ref().clone();
        circuit.replace_first_guard_polynomial_for_test(polynomial);
        let circuit = Arc::new(circuit);
        let duplicate = CanonicalRebasedCandidate::new(
            circuit.clone(),
            source.anchor.clone(),
            source.primary_probe.clone(),
            source.supporting_probes.to_vec(),
        );
        let mut candidates = Vec::from(std::mem::take(&mut self.candidates));
        candidates.push(duplicate);
        candidates.sort_unstable_by(|left, right| {
            compare_exact_circuit_content(left.circuit(), right.circuit())
        });
        self.candidates = candidates.into_boxed_slice();
        circuit
    }

    pub(super) fn new(
        epoch: Arc<FreshTaskEpoch>,
        candidates: Vec<CanonicalRebasedCandidate>,
        attempts: Vec<CanonicalRebaseAttempt>,
        telemetry: CanonicalReplayTelemetry,
    ) -> Self {
        Self {
            epoch,
            candidates: candidates.into_boxed_slice(),
            attempts: attempts.into_boxed_slice(),
            telemetry,
        }
    }
}

/// Result of one canonical replay transaction.  Neither empty variant grants
/// terminal or no-relation authority.
#[derive(Debug)]
pub(crate) enum CanonicalReplayDisposition {
    NoReplayedNominations,
    NoRebasedCircuits {
        epoch: Arc<FreshTaskEpoch>,
        attempts: Box<[CanonicalRebaseAttempt]>,
        telemetry: CanonicalReplayTelemetry,
    },
    Rebased(CanonicalReplayBatch),
}
