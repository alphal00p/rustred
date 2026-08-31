use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::{ExactCircuitError, ExactCircuitLift};
use crate::foundry::completion::frame::modular::{
    ModularHit, ModularKernelError, ModularRankDiagnostics, ModularRightObstruction,
    ModularSampleFingerprint,
};
use crate::identity::IntegralShift;

use super::super::{PhysicalFramePlan, SourceInstanceId};
use super::TargetEvidenceLimits;

/// Whether one explicit modular task may nominate an exact proposal or is
/// reserved for held-out telemetry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum EvidenceProbeRole {
    Discovery,
    HeldOut,
}

/// Borrowed ingress for one probe task. The admitted plan copies both points
/// only after checking its aggregate resource policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceProbeSpec<'point> {
    role: EvidenceProbeRole,
    modulus: u64,
    base_parameters: &'point [i64],
    chart_coordinates: &'point [u64],
}

impl<'point> EvidenceProbeSpec<'point> {
    pub(crate) const fn new(
        role: EvidenceProbeRole,
        modulus: u64,
        base_parameters: &'point [i64],
        chart_coordinates: &'point [u64],
    ) -> Self {
        Self {
            role,
            modulus,
            base_parameters,
            chart_coordinates,
        }
    }

    pub(crate) const fn role(self) -> EvidenceProbeRole {
        self.role
    }

    pub(crate) const fn modulus(self) -> u64 {
        self.modulus
    }

    pub(crate) const fn base_parameters(self) -> &'point [i64] {
        self.base_parameters
    }

    pub(crate) const fn chart_coordinates(self) -> &'point [u64] {
        self.chart_coordinates
    }
}

/// One fully admitted, ordinal-stable modular task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EvidenceProbe {
    role: EvidenceProbeRole,
    modulus: u64,
    base_parameters: Box<[i64]>,
    chart_coordinates: Box<[u64]>,
}

impl EvidenceProbe {
    pub(crate) const fn role(&self) -> EvidenceProbeRole {
        self.role
    }

    pub(crate) const fn modulus(&self) -> u64 {
        self.modulus
    }

    pub(crate) fn base_parameters(&self) -> &[i64] {
        &self.base_parameters
    }

    pub(crate) fn chart_coordinates(&self) -> &[u64] {
        &self.chart_coordinates
    }

    pub(super) fn from_parts(
        role: EvidenceProbeRole,
        modulus: u64,
        base_parameters: Vec<i64>,
        chart_coordinates: Vec<u64>,
    ) -> Self {
        Self {
            role,
            modulus,
            base_parameters: base_parameters.into_boxed_slice(),
            chart_coordinates: chart_coordinates.into_boxed_slice(),
        }
    }
}

/// Finite probe plan already bound to one physical frame and coefficient
/// context. Probe order is execution and report order.
#[derive(Debug)]
pub(crate) struct EvidenceProbePlan<'context, 'frame> {
    pub(super) context: &'context IndexedCoefficientContext,
    pub(super) frame: &'frame PhysicalFramePlan,
    pub(super) probes: Box<[EvidenceProbe]>,
    pub(super) limits: TargetEvidenceLimits,
}

impl<'context, 'frame> EvidenceProbePlan<'context, 'frame> {
    pub(crate) fn try_new<'point>(
        context: &'context IndexedCoefficientContext,
        frame: &'frame PhysicalFramePlan,
        probes: impl IntoIterator<Item = EvidenceProbeSpec<'point>>,
        limits: TargetEvidenceLimits,
    ) -> Result<Self, super::TargetEvidenceError> {
        super::schedule::admit_probe_plan(context, frame, probes, limits)
    }

    pub(crate) const fn context(&self) -> &'context IndexedCoefficientContext {
        self.context
    }

    pub(crate) const fn frame(&self) -> &'frame PhysicalFramePlan {
        self.frame
    }

    pub(crate) fn probes(&self) -> &[EvidenceProbe] {
        &self.probes
    }

    pub(crate) const fn limits(&self) -> TargetEvidenceLimits {
        self.limits
    }
}

impl PartialEq for EvidenceProbePlan<'_, '_> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.context, other.context)
            && std::ptr::eq(self.frame, other.frame)
            && self.probes == other.probes
            && self.limits == other.limits
    }
}

impl Eq for EvidenceProbePlan<'_, '_> {}

/// Exact, frame-local identity of the source/pivot chronology observed by one
/// target query. Fill counters and sample values are deliberately excluded.
/// The target and forbidden set are included so this key cannot silently be
/// reused for another target-local query.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CanonicalTraceIdentity {
    target_column: usize,
    forbidden_columns: Arc<[usize]>,
    forbidden_rank: usize,
    augmented_rank: usize,
    forbidden_pivot_shifts: Box<[IntegralShift]>,
    augmented_pivot_shifts: Box<[IntegralShift]>,
    forbidden_source_instances: Box<[SourceInstanceId]>,
    augmented_source_instances: Box<[SourceInstanceId]>,
}

impl CanonicalTraceIdentity {
    pub(crate) const fn target_column(&self) -> usize {
        self.target_column
    }

    pub(crate) fn forbidden_columns(&self) -> &[usize] {
        &self.forbidden_columns
    }

    pub(crate) const fn forbidden_rank(&self) -> usize {
        self.forbidden_rank
    }

    pub(crate) const fn augmented_rank(&self) -> usize {
        self.augmented_rank
    }

    pub(crate) fn forbidden_pivot_shifts(&self) -> &[IntegralShift] {
        &self.forbidden_pivot_shifts
    }

    pub(crate) fn augmented_pivot_shifts(&self) -> &[IntegralShift] {
        &self.augmented_pivot_shifts
    }

    pub(crate) fn forbidden_source_instances(&self) -> &[SourceInstanceId] {
        &self.forbidden_source_instances
    }

    pub(crate) fn augmented_source_instances(&self) -> &[SourceInstanceId] {
        &self.augmented_source_instances
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_parts(
        target_column: usize,
        forbidden_columns: Arc<[usize]>,
        forbidden_rank: usize,
        augmented_rank: usize,
        forbidden_pivot_shifts: Vec<IntegralShift>,
        augmented_pivot_shifts: Vec<IntegralShift>,
        forbidden_source_instances: Vec<SourceInstanceId>,
        augmented_source_instances: Vec<SourceInstanceId>,
    ) -> Self {
        Self {
            target_column,
            forbidden_columns,
            forbidden_rank,
            augmented_rank,
            forbidden_pivot_shifts: forbidden_pivot_shifts.into_boxed_slice(),
            augmented_pivot_shifts: augmented_pivot_shifts.into_boxed_slice(),
            forbidden_source_instances: forbidden_source_instances.into_boxed_slice(),
            augmented_source_instances: augmented_source_instances.into_boxed_slice(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProbeRejectionStage {
    Sample,
    Query,
}

/// Complete scheduler-owned result for one probe ordinal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EvidenceProbeOutcome<'frame> {
    RejectedSample {
        error: ModularKernelError,
    },
    RejectedQuery {
        sample: Arc<ModularSampleFingerprint>,
        error: ModularKernelError,
    },
    ModularNoHit {
        sample: Arc<ModularSampleFingerprint>,
        obstruction: ModularRightObstruction<'frame>,
    },
    Hit {
        hit: ModularHit<'frame>,
        trace: Arc<CanonicalTraceIdentity>,
    },
}

impl<'frame> EvidenceProbeOutcome<'frame> {
    pub(crate) const fn rejection_stage(&self) -> Option<ProbeRejectionStage> {
        match self {
            Self::RejectedSample { .. } => Some(ProbeRejectionStage::Sample),
            Self::RejectedQuery { .. } => Some(ProbeRejectionStage::Query),
            Self::ModularNoHit { .. } | Self::Hit { .. } => None,
        }
    }

    pub(crate) const fn rejection(&self) -> Option<&ModularKernelError> {
        match self {
            Self::RejectedSample { error } | Self::RejectedQuery { error, .. } => Some(error),
            Self::ModularNoHit { .. } | Self::Hit { .. } => None,
        }
    }

    pub(crate) fn sample_fingerprint(&self) -> Option<&Arc<ModularSampleFingerprint>> {
        match self {
            Self::RejectedSample { .. } => None,
            Self::RejectedQuery { sample, .. } | Self::ModularNoHit { sample, .. } => Some(sample),
            Self::Hit { hit, .. } => Some(hit.sample_fingerprint()),
        }
    }

    pub(crate) const fn diagnostics(&self) -> Option<&ModularRankDiagnostics> {
        match self {
            Self::RejectedSample { .. } | Self::RejectedQuery { .. } => None,
            Self::ModularNoHit { obstruction, .. } => Some(obstruction.diagnostics()),
            Self::Hit { hit, .. } => Some(hit.diagnostics()),
        }
    }

    pub(crate) const fn trace(&self) -> Option<&Arc<CanonicalTraceIdentity>> {
        match self {
            Self::Hit { trace, .. } => Some(trace),
            _ => None,
        }
    }

    pub(crate) const fn hit(&self) -> Option<&ModularHit<'frame>> {
        match self {
            Self::Hit { hit, .. } => Some(hit),
            _ => None,
        }
    }
}

/// One exact trace class among Discovery hits only. Membership is telemetry;
/// no coefficient is combined across probes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DiscoveryTraceGroup {
    trace: Arc<CanonicalTraceIdentity>,
    probe_ordinals: Box<[usize]>,
}

impl DiscoveryTraceGroup {
    pub(crate) const fn trace(&self) -> &Arc<CanonicalTraceIdentity> {
        &self.trace
    }

    pub(crate) fn probe_ordinals(&self) -> &[usize] {
        &self.probe_ordinals
    }

    pub(super) fn new(trace: Arc<CanonicalTraceIdentity>, probe_ordinals: Vec<usize>) -> Self {
        Self {
            trace,
            probe_ordinals: probe_ordinals.into_boxed_slice(),
        }
    }
}

/// Exact check of at most one Discovery proposal. Even a replayed circuit is
/// only the guarded exact relation returned by the exact layer, not closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExactProposalOutcome {
    NoDiscoveryHit,
    Checked {
        probe_ordinal: usize,
        trace: Arc<CanonicalTraceIdentity>,
        result: Result<ExactCircuitLift, ExactCircuitError>,
    },
}

impl ExactProposalOutcome {
    pub(crate) const fn probe_ordinal(&self) -> Option<usize> {
        match self {
            Self::NoDiscoveryHit => None,
            Self::Checked { probe_ordinal, .. } => Some(*probe_ordinal),
        }
    }

    pub(crate) const fn trace(&self) -> Option<&Arc<CanonicalTraceIdentity>> {
        match self {
            Self::NoDiscoveryHit => None,
            Self::Checked { trace, .. } => Some(trace),
        }
    }

    pub(crate) const fn result(&self) -> Option<&Result<ExactCircuitLift, ExactCircuitError>> {
        match self {
            Self::NoDiscoveryHit => None,
            Self::Checked { result, .. } => Some(result),
        }
    }
}

/// Interpretation of one HeldOut outcome relative to the selected Discovery
/// trace. None of these variants changes the exact proposal result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeldOutAssessment {
    NoSelectedDiscoveryTrace,
    RejectedSample,
    RejectedQuery,
    ModularNoHit,
    TraceMatch,
    TraceMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct HeldOutDiagnostic {
    probe_ordinal: usize,
    assessment: HeldOutAssessment,
}

impl HeldOutDiagnostic {
    pub(crate) const fn probe_ordinal(self) -> usize {
        self.probe_ordinal
    }

    pub(crate) const fn assessment(self) -> HeldOutAssessment {
        self.assessment
    }

    pub(super) const fn new(probe_ordinal: usize, assessment: HeldOutAssessment) -> Self {
        Self {
            probe_ordinal,
            assessment,
        }
    }
}

/// Ordered result of one serial target-evidence run.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TargetEvidenceReport<'context, 'frame> {
    probe_plan: EvidenceProbePlan<'context, 'frame>,
    outcomes: Box<[EvidenceProbeOutcome<'frame>]>,
    discovery_groups: Box<[DiscoveryTraceGroup]>,
    exact_proposal: ExactProposalOutcome,
    held_out: Box<[HeldOutDiagnostic]>,
    held_out_trace_stable: bool,
}

impl<'context, 'frame> TargetEvidenceReport<'context, 'frame> {
    pub(crate) const fn probe_plan(&self) -> &EvidenceProbePlan<'context, 'frame> {
        &self.probe_plan
    }

    pub(crate) fn outcomes(&self) -> &[EvidenceProbeOutcome<'frame>] {
        &self.outcomes
    }

    pub(crate) fn discovery_groups(&self) -> &[DiscoveryTraceGroup] {
        &self.discovery_groups
    }

    pub(crate) const fn exact_proposal(&self) -> &ExactProposalOutcome {
        &self.exact_proposal
    }

    pub(crate) fn held_out_diagnostics(&self) -> &[HeldOutDiagnostic] {
        &self.held_out
    }

    /// True only when at least one HeldOut task exists and every HeldOut task
    /// produced a Hit with the selected Discovery trace.
    pub(crate) const fn held_out_trace_stable(&self) -> bool {
        self.held_out_trace_stable
    }

    pub(super) fn from_parts(
        probe_plan: EvidenceProbePlan<'context, 'frame>,
        outcomes: Vec<EvidenceProbeOutcome<'frame>>,
        discovery_groups: Vec<DiscoveryTraceGroup>,
        exact_proposal: ExactProposalOutcome,
        held_out: Vec<HeldOutDiagnostic>,
        held_out_trace_stable: bool,
    ) -> Self {
        Self {
            probe_plan,
            outcomes: outcomes.into_boxed_slice(),
            discovery_groups: discovery_groups.into_boxed_slice(),
            exact_proposal,
            held_out: held_out.into_boxed_slice(),
            held_out_trace_stable,
        }
    }
}
