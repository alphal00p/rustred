use crate::family::IntegralKey;
use crate::foundry::completion::source_discovery::{
    ExactOwnerCoverSnapshot, ExactOwnerLedgerSnapshotIdentity, ExactTerminalCoverDelta,
};
use crate::foundry::completion::stratum::DecoratedStratumId;

use super::super::{
    SpiredEqualityStepCensus, SpiredEqualityStepCommitted, SpiredEqualityStepIncomplete,
};

/// Explicit finite inputs from which every case-local modular probe is built.
///
/// A chart-rank point chooses one rank inside every coordinate interval; it
/// is not a lattice point retained by the logical case. Fixed equality axes
/// always override it. Explicit vectors avoid trapping every modular sample
/// on one affine line while keeping probe order deterministic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialProbePortfolio {
    pub(super) moduli: Box<[u64]>,
    pub(super) base_parameter_points: Box<[Box<[i64]>]>,
    pub(super) chart_rank_points: Box<[Box<[u64]>]>,
    pub(super) probe_template_count: usize,
}

impl SpiredSerialProbePortfolio {
    pub(crate) fn moduli(&self) -> &[u64] {
        &self.moduli
    }

    pub(crate) fn base_parameter_points(&self) -> &[Box<[i64]>] {
        &self.base_parameter_points
    }

    pub(crate) fn chart_rank_points(&self) -> &[Box<[u64]>] {
        &self.chart_rank_points
    }

    pub(crate) const fn probe_template_count(&self) -> usize {
        self.probe_template_count
    }
}

/// Exact work needed to materialize one case's probe portfolio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredSerialProbeCensus {
    pub(super) free_axes: usize,
    pub(super) fixed_axes: usize,
    pub(super) probes_generated: usize,
    pub(super) retained_coordinate_cells: usize,
}

impl SpiredSerialProbeCensus {
    pub(crate) const fn free_axes(self) -> usize {
        self.free_axes
    }

    pub(crate) const fn fixed_axes(self) -> usize {
        self.fixed_axes
    }

    pub(crate) const fn probes_generated(self) -> usize {
        self.probes_generated
    }

    pub(crate) const fn retained_coordinate_cells(self) -> usize {
        self.retained_coordinate_cells
    }
}

/// Where the exact finite terminal authority came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredExistingTerminalAuthority {
    AlreadyRetainedByLedger,
    AuthenticatedByPredecessor,
}

/// Normal non-authoritative stop for the current exact case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialCaseIncomplete {
    EqualityStep(SpiredEqualityStepIncomplete),
    ZeroDimensionalCaseNeedsDeclaredTerminal { integral: IntegralKey },
}

/// Effect of one current-case attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialCaseOutcome {
    RuleCommitted(SpiredEqualityStepCommitted),
    ExistingTerminalRetired {
        integral: IntegralKey,
        authority: SpiredExistingTerminalAuthority,
        delta: ExactTerminalCoverDelta,
    },
    Incomplete(SpiredSerialCaseIncomplete),
}

/// Bounded audit record for one deterministic generic-first case attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialCaseReport {
    declared_carrier_id: DecoratedStratumId,
    case_id: DecoratedStratumId,
    free_dimension: usize,
    probe_census: SpiredSerialProbeCensus,
    equality_census: Option<SpiredEqualityStepCensus>,
    outcome: SpiredSerialCaseOutcome,
}

impl SpiredSerialCaseReport {
    pub(crate) const fn declared_carrier_id(&self) -> &DecoratedStratumId {
        &self.declared_carrier_id
    }

    pub(crate) const fn case_id(&self) -> &DecoratedStratumId {
        &self.case_id
    }

    pub(crate) const fn free_dimension(&self) -> usize {
        self.free_dimension
    }

    pub(crate) const fn probe_census(&self) -> SpiredSerialProbeCensus {
        self.probe_census
    }

    pub(crate) const fn equality_census(&self) -> Option<SpiredEqualityStepCensus> {
        self.equality_census
    }

    pub(crate) const fn outcome(&self) -> &SpiredSerialCaseOutcome {
        &self.outcome
    }

    pub(super) fn new(
        declared_carrier_id: DecoratedStratumId,
        case_id: DecoratedStratumId,
        free_dimension: usize,
        probe_census: SpiredSerialProbeCensus,
        equality_census: Option<SpiredEqualityStepCensus>,
        outcome: SpiredSerialCaseOutcome,
    ) -> Self {
        Self {
            declared_carrier_id,
            case_id,
            free_dimension,
            probe_census,
            equality_census,
            outcome,
        }
    }
}

/// The present driver does not retain a source-shell cursor across calls.
///
/// This explicit value prevents callers from mistaking a resumable exact
/// obligation for a resumed algebra stream.  A later diagonal/source cursor
/// can replace this policy without changing case geometry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialResumePolicy {
    RestartCurrentCaseFromFirstConfiguredDiagonalAndDepthZero,
}

/// Complete cumulative scalar telemetry for one serial drive invocation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredSerialDriverCensus {
    pub(super) case_attempts: usize,
    pub(super) positive_dimensional_case_attempts: usize,
    pub(super) zero_dimensional_case_attempts: usize,
    pub(super) rule_commits: usize,
    pub(super) existing_terminal_retirements: usize,
    pub(super) predecessor_terminals_retained: usize,
    pub(super) finite_residual_reconciliations: usize,
    pub(super) finite_residual_points_reified: usize,
    pub(super) incomplete_case_attempts: usize,
    pub(super) generated_probes: usize,
    pub(super) generated_probe_coordinate_cells: usize,
    pub(super) probe_attempts: usize,
    pub(super) singular_probes_skipped: usize,
    pub(super) scheduled_requests: usize,
    pub(super) streamed_rows: usize,
    pub(super) modular_hits: usize,
    pub(super) exact_lift_attempts: usize,
    pub(super) admitted_candidates: usize,
    pub(super) exact_guard_cases: usize,
    pub(super) owner_compile_attempts: usize,
    pub(super) retained_case_reports: usize,
}

impl SpiredSerialDriverCensus {
    pub(crate) const fn case_attempts(self) -> usize {
        self.case_attempts
    }

    pub(crate) const fn positive_dimensional_case_attempts(self) -> usize {
        self.positive_dimensional_case_attempts
    }

    pub(crate) const fn zero_dimensional_case_attempts(self) -> usize {
        self.zero_dimensional_case_attempts
    }

    pub(crate) const fn rule_commits(self) -> usize {
        self.rule_commits
    }

    pub(crate) const fn existing_terminal_retirements(self) -> usize {
        self.existing_terminal_retirements
    }

    pub(crate) const fn predecessor_terminals_retained(self) -> usize {
        self.predecessor_terminals_retained
    }

    pub(crate) const fn finite_residual_reconciliations(self) -> usize {
        self.finite_residual_reconciliations
    }

    pub(crate) const fn finite_residual_points_reified(self) -> usize {
        self.finite_residual_points_reified
    }

    pub(crate) const fn incomplete_case_attempts(self) -> usize {
        self.incomplete_case_attempts
    }

    pub(crate) const fn generated_probes(self) -> usize {
        self.generated_probes
    }

    pub(crate) const fn generated_probe_coordinate_cells(self) -> usize {
        self.generated_probe_coordinate_cells
    }

    pub(crate) const fn probe_attempts(self) -> usize {
        self.probe_attempts
    }

    pub(crate) const fn singular_probes_skipped(self) -> usize {
        self.singular_probes_skipped
    }

    pub(crate) const fn scheduled_requests(self) -> usize {
        self.scheduled_requests
    }

    pub(crate) const fn streamed_rows(self) -> usize {
        self.streamed_rows
    }

    pub(crate) const fn modular_hits(self) -> usize {
        self.modular_hits
    }

    pub(crate) const fn exact_lift_attempts(self) -> usize {
        self.exact_lift_attempts
    }

    pub(crate) const fn admitted_candidates(self) -> usize {
        self.admitted_candidates
    }

    pub(crate) const fn exact_guard_cases(self) -> usize {
        self.exact_guard_cases
    }

    pub(crate) const fn owner_compile_attempts(self) -> usize {
        self.owner_compile_attempts
    }

    pub(crate) const fn retained_case_reports(self) -> usize {
        self.retained_case_reports
    }
}

/// Why a resumable drive stopped without a hard error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialDriverStop {
    CompilerClosed {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
        pending_cases: usize,
    },
    /// The exact compiler covered the ledger's deliberately retained proper
    /// subcarrier. This is useful diagnostic evidence but is not whole-sector
    /// closure authority.
    BoundedCarrierClosed {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
    },
    WorklistExhausted {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
    },
    FiniteResidualIncomplete {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
        integrals: Box<[IntegralKey]>,
        reason: SpiredSerialFiniteResidualReason,
        resume: SpiredSerialResumePolicy,
    },
    CurrentCaseIncomplete {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
        case_id: DecoratedStratumId,
        reason: SpiredSerialCaseIncomplete,
        resume: SpiredSerialResumePolicy,
    },
    ResourceLimit {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
        current_case_id: Option<DecoratedStratumId>,
        resource: &'static str,
        requested: usize,
        limit: usize,
        resume: SpiredSerialResumePolicy,
    },
}

impl SpiredSerialDriverStop {
    pub(crate) const fn authority(&self) -> &ExactOwnerLedgerSnapshotIdentity {
        match self {
            Self::CompilerClosed { authority, .. }
            | Self::BoundedCarrierClosed { authority, .. }
            | Self::WorklistExhausted { authority, .. }
            | Self::FiniteResidualIncomplete { authority, .. }
            | Self::CurrentCaseIncomplete { authority, .. }
            | Self::ResourceLimit { authority, .. } => authority,
        }
    }

    pub(crate) const fn snapshot(&self) -> ExactOwnerCoverSnapshot {
        match self {
            Self::CompilerClosed { snapshot, .. }
            | Self::BoundedCarrierClosed { snapshot, .. }
            | Self::WorklistExhausted { snapshot, .. }
            | Self::FiniteResidualIncomplete { snapshot, .. }
            | Self::CurrentCaseIncomplete { snapshot, .. }
            | Self::ResourceLimit { snapshot, .. } => *snapshot,
        }
    }
}

/// Why exact finite compiler residuals could not become logical cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredSerialFiniteResidualReason {
    NoRetainedDeclaredCarrier,
    OutsideRetainedDeclaredCarrier,
    NeedsDeclaredTerminalAuthority,
}

/// Bounded result of one resumable serial drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredSerialDriverReport {
    census: SpiredSerialDriverCensus,
    cases: Box<[SpiredSerialCaseReport]>,
    stop: SpiredSerialDriverStop,
}

impl SpiredSerialDriverReport {
    pub(crate) const fn census(&self) -> SpiredSerialDriverCensus {
        self.census
    }

    pub(crate) fn cases(&self) -> &[SpiredSerialCaseReport] {
        &self.cases
    }

    pub(crate) const fn stop(&self) -> &SpiredSerialDriverStop {
        &self.stop
    }

    pub(super) fn new(
        census: SpiredSerialDriverCensus,
        cases: Vec<SpiredSerialCaseReport>,
        stop: SpiredSerialDriverStop,
    ) -> Self {
        Self {
            census,
            cases: cases.into_boxed_slice(),
            stop,
        }
    }
}
