use std::sync::Arc;

use crate::foundry::completion::source_discovery::leader_walk::LeaderWalkTask;
use crate::foundry::completion::source_discovery::{
    CanonicalExactOwnerLedger, ExactOwnerCoverSnapshot, ExactOwnerLedgerSnapshotIdentity,
    ExactSemanticExecutableOwner,
};

use super::{SpiredFixedPointError, SpiredTargetPortfolioBudget, SpiredTargetRunnerError};

/// Normal, non-authoritative reason why one bounded target portfolio did not
/// produce an executable owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredTargetPortfolioIncompleteReason {
    SearchDepthExhausted,
    ProbePortfolioExhausted,
    AlternativePortfolioExhausted,
    CompactReplayInconclusive,
    BlockedByKnownZero,
    GuardWitnessUnavailable,
    GuardedStratumUnsupported,
}

/// Complete aggregate work performed inside one target portfolio.
///
/// In particular, alternative nodes and rejected hits are explicit. This
/// prevents a future first-hit resume implementation from hiding an
/// exponential exclusion walk behind one nominal target attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredTargetPortfolioCensus {
    pub(crate) target_lane_runs: usize,
    pub(crate) probe_attempts: usize,
    pub(crate) scheduled_requests: usize,
    pub(crate) streamed_rows: usize,
    pub(crate) modular_hits: usize,
    pub(crate) exact_lift_attempts: usize,
    pub(crate) rejected_hits: usize,
    pub(crate) alternative_nodes: usize,
    pub(crate) excluded_requests: usize,
    pub(crate) excluded_request_coordinate_cells: usize,
    pub(crate) guard_predicates: usize,
    pub(crate) guard_obligations: usize,
    pub(crate) owner_compile_attempts: usize,
}

impl SpiredTargetPortfolioCensus {
    pub(crate) fn fits(
        self,
        budget: SpiredTargetPortfolioBudget,
    ) -> Result<(), SpiredFixedPointError> {
        check_reported(
            "target lane runs",
            self.target_lane_runs,
            budget.target_lane_runs,
        )?;
        check_reported("probe attempts", self.probe_attempts, budget.probe_attempts)?;
        check_reported(
            "scheduled requests",
            self.scheduled_requests,
            budget.scheduled_requests,
        )?;
        check_reported("streamed rows", self.streamed_rows, budget.streamed_rows)?;
        check_reported("modular hits", self.modular_hits, budget.modular_hits)?;
        check_reported(
            "exact lift attempts",
            self.exact_lift_attempts,
            budget.exact_lift_attempts,
        )?;
        check_reported("rejected hits", self.rejected_hits, budget.rejected_hits)?;
        check_reported(
            "alternative nodes",
            self.alternative_nodes,
            budget.alternative_nodes,
        )?;
        check_reported(
            "excluded requests",
            self.excluded_requests,
            budget.excluded_requests,
        )?;
        check_reported(
            "excluded request coordinate cells",
            self.excluded_request_coordinate_cells,
            budget.excluded_request_coordinate_cells,
        )?;
        check_reported(
            "guard predicates",
            self.guard_predicates,
            budget.guard_predicates,
        )?;
        check_reported(
            "guard obligations",
            self.guard_obligations,
            budget.guard_obligations,
        )?;
        check_reported(
            "owner compile attempts",
            self.owner_compile_attempts,
            budget.owner_compile_attempts,
        )
    }
}

fn check_reported(
    resource: &'static str,
    reported: usize,
    remaining: usize,
) -> Result<(), SpiredFixedPointError> {
    if reported > remaining {
        Err(SpiredFixedPointError::RunnerExceededBudget {
            resource,
            reported,
            remaining,
        })
    } else {
        Ok(())
    }
}

/// Normal outcome of one complete, bounded target portfolio.
#[derive(Debug)]
pub(crate) enum SpiredTargetPortfolioDisposition {
    CompiledOwner(Arc<ExactSemanticExecutableOwner>),
    Incomplete(SpiredTargetPortfolioIncompleteReason),
}

#[derive(Debug)]
pub(crate) struct SpiredTargetPortfolioReport {
    census: SpiredTargetPortfolioCensus,
    disposition: SpiredTargetPortfolioDisposition,
}

impl SpiredTargetPortfolioReport {
    pub(crate) const fn new(
        census: SpiredTargetPortfolioCensus,
        disposition: SpiredTargetPortfolioDisposition,
    ) -> Self {
        Self {
            census,
            disposition,
        }
    }

    pub(crate) const fn census(&self) -> SpiredTargetPortfolioCensus {
        self.census
    }

    pub(crate) fn into_disposition(self) -> SpiredTargetPortfolioDisposition {
        self.disposition
    }
}

/// Borrowed authority supplied to one target portfolio.
///
/// The runner receives the live ledger read-only. Only this coordinator may
/// apply an owner after rejoining the separately retained opaque snapshot.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpiredFixedPointTarget<'a> {
    task: &'a LeaderWalkTask,
    ledger: &'a CanonicalExactOwnerLedger,
}

impl<'a> SpiredFixedPointTarget<'a> {
    pub(super) const fn new(
        task: &'a LeaderWalkTask,
        ledger: &'a CanonicalExactOwnerLedger,
    ) -> Self {
        Self { task, ledger }
    }

    pub(crate) const fn task(&self) -> &'a LeaderWalkTask {
        self.task
    }

    pub(crate) const fn ledger(&self) -> &'a CanonicalExactOwnerLedger {
        self.ledger
    }
}

/// Private integration seam for streamed target discovery.
///
/// Its production implementation must own ordered probe retry and bounded
/// exclusion-branch first-hit resume. A single call represents the complete
/// bounded portfolio, not merely the first modular hit. It must also build the
/// coordinate case exactly: `None` chart endpoints remain symbolic
/// carrier-reaching tails, while coefficient probes use a separate contained
/// anchor with active `x -> n=x+1` and inactive `x -> n=-x` conversion. This
/// coordinator deliberately does not approximate that geometry.
pub(crate) trait SpiredFixedPointTargetRunner {
    fn try_run_target_portfolio(
        &mut self,
        target: SpiredFixedPointTarget<'_>,
        budget: SpiredTargetPortfolioBudget,
    ) -> Result<SpiredTargetPortfolioReport, SpiredTargetRunnerError>;
}

/// Cumulative scalar telemetry for one coordinator drive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SpiredFixedPointCensus {
    pub(super) epochs: usize,
    pub(super) plans: usize,
    pub(super) planned_targets: usize,
    pub(super) targets_inspected: usize,
    pub(super) terminal_targets_skipped: usize,
    pub(super) target_portfolios: usize,
    pub(super) target_work: SpiredTargetPortfolioCensus,
    pub(super) incomplete_target_portfolios: usize,
    pub(super) duplicate_owner_proposals: usize,
    pub(super) semantic_owner_mutations: usize,
    pub(super) strict_geometric_shrinks: usize,
}

impl SpiredFixedPointCensus {
    pub(crate) const fn epochs(self) -> usize {
        self.epochs
    }

    pub(crate) const fn plans(self) -> usize {
        self.plans
    }

    pub(crate) const fn planned_targets(self) -> usize {
        self.planned_targets
    }

    pub(crate) const fn targets_inspected(self) -> usize {
        self.targets_inspected
    }

    pub(crate) const fn terminal_targets_skipped(self) -> usize {
        self.terminal_targets_skipped
    }

    pub(crate) const fn target_portfolios(self) -> usize {
        self.target_portfolios
    }

    pub(crate) const fn target_work(self) -> SpiredTargetPortfolioCensus {
        self.target_work
    }

    pub(crate) const fn incomplete_target_portfolios(self) -> usize {
        self.incomplete_target_portfolios
    }

    pub(crate) const fn duplicate_owner_proposals(self) -> usize {
        self.duplicate_owner_proposals
    }

    pub(crate) const fn semantic_owner_mutations(self) -> usize {
        self.semantic_owner_mutations
    }

    pub(crate) const fn strict_geometric_shrinks(self) -> usize {
        self.strict_geometric_shrinks
    }
}

/// Why a live nonclosed ledger left one bounded fixed-point drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpiredFixedPointIncompleteReason {
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    FiniteResidualRequiresExactWork {
        uncovered_boxes: usize,
        missing_terminals: usize,
        guard_incomplete_owners: usize,
    },
    StableProgramExhausted {
        first_target_reason: Option<SpiredTargetPortfolioIncompleteReason>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredFixedPointStop {
    CompilerClosed {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
    },
    Incomplete {
        authority: ExactOwnerLedgerSnapshotIdentity,
        snapshot: ExactOwnerCoverSnapshot,
        reason: SpiredFixedPointIncompleteReason,
    },
}

impl SpiredFixedPointStop {
    pub(crate) const fn authority(&self) -> &ExactOwnerLedgerSnapshotIdentity {
        match self {
            Self::CompilerClosed { authority, .. } | Self::Incomplete { authority, .. } => {
                authority
            }
        }
    }

    pub(crate) const fn snapshot(&self) -> ExactOwnerCoverSnapshot {
        match self {
            Self::CompilerClosed { snapshot, .. } | Self::Incomplete { snapshot, .. } => *snapshot,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpiredFixedPointReport {
    census: SpiredFixedPointCensus,
    stop: SpiredFixedPointStop,
}

impl SpiredFixedPointReport {
    pub(crate) const fn census(&self) -> SpiredFixedPointCensus {
        self.census
    }

    pub(crate) const fn stop(&self) -> &SpiredFixedPointStop {
        &self.stop
    }

    pub(super) const fn new(census: SpiredFixedPointCensus, stop: SpiredFixedPointStop) -> Self {
        Self { census, stop }
    }
}
