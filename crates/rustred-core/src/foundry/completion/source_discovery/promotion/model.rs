use std::sync::Arc;

use crate::foundry::cell::{RuleCell, RuleCellGuardDomainSplit};
use crate::foundry::completion::frame::admission::ExactGuardRefinement;
use crate::foundry::completion::frame::exact::{ClearedExactCircuit, ExactTargetCircuit};
use crate::foundry::completion::source_discovery::FreshTaskEpoch;
use crate::foundry::completion::stratum::GuardBranchIdentity;

/// Exact authority and executable payload minted in one transaction.
///
/// Keeping the epoch is intentional: the circuit's physical ordinals and
/// lower-owner witnesses must never outlive the plan and immutable snapshot
/// that give them meaning.
#[derive(Debug)]
pub(crate) struct AdmittedExactRuleCandidate {
    epoch: Arc<FreshTaskEpoch>,
    circuit: Arc<ExactTargetCircuit>,
    cleared: Arc<ClearedExactCircuit>,
    cell: Arc<RuleCell>,
    guard_refinement: ExactGuardRefinement,
    guard_domain_split: Option<RuleCellGuardDomainSplit>,
}

impl AdmittedExactRuleCandidate {
    pub(crate) fn epoch(&self) -> &Arc<FreshTaskEpoch> {
        &self.epoch
    }

    pub(crate) fn circuit(&self) -> &Arc<ExactTargetCircuit> {
        &self.circuit
    }

    pub(crate) fn cleared(&self) -> &Arc<ClearedExactCircuit> {
        &self.cleared
    }

    /// Borrow the executable payload without allowing it to outlive the
    /// exact epoch/circuit authority retained by this candidate.
    pub(crate) fn cell(&self) -> &RuleCell {
        self.cell.as_ref()
    }

    /// Retain the exact executable payload without copying or rebuilding its
    /// authenticated source views and rule evidence.
    pub(crate) const fn cell_owner(&self) -> &Arc<RuleCell> {
        &self.cell
    }

    pub(crate) const fn guard_refinement(&self) -> &ExactGuardRefinement {
        &self.guard_refinement
    }

    pub(crate) const fn guard_domain_split(&self) -> Option<&RuleCellGuardDomainSplit> {
        self.guard_domain_split.as_ref()
    }

    pub(super) fn new(
        epoch: Arc<FreshTaskEpoch>,
        circuit: Arc<ExactTargetCircuit>,
        cleared: Arc<ClearedExactCircuit>,
        cell: Arc<RuleCell>,
        guard_refinement: ExactGuardRefinement,
        guard_domain_split: Option<RuleCellGuardDomainSplit>,
    ) -> Self {
        Self {
            epoch,
            circuit,
            cleared,
            cell,
            guard_refinement,
            guard_domain_split,
        }
    }
}

/// Why a valid exact identity cannot own its whole carrier box as an ordinary
/// `RuleCell`.  A later semantic-routing layer must restrict ownership to the
/// all-nonzero stratum and retain every zero-branch obligation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExactRuleCellGuardObstruction {
    IntegerRoot {
        guard_ordinal: usize,
        position: usize,
        value: i64,
    },
    UnsupportedMultivariate {
        guard_ordinal: usize,
    },
}

/// Normal semantic outcomes of one otherwise valid replayed proposal.
#[derive(Debug)]
pub(crate) enum ExactRuleCellPromotionDisposition {
    Admitted(AdmittedExactRuleCandidate),
    BlockedByKnownZero {
        epoch: Arc<FreshTaskEpoch>,
        circuit: Arc<ExactTargetCircuit>,
        cleared: Arc<ClearedExactCircuit>,
        required_predicate_ordinal: usize,
        first_circuit_guard_ordinal: usize,
        zero_branch: GuardBranchIdentity,
    },
    NeedsGuardedStratum {
        epoch: Arc<FreshTaskEpoch>,
        circuit: Arc<ExactTargetCircuit>,
        cleared: Arc<ClearedExactCircuit>,
        refinement: ExactGuardRefinement,
        obstruction: ExactRuleCellGuardObstruction,
    },
    /// The exact identity is valid, but the caller selected a concrete replay
    /// anchor on one of its nonzero guards.  Promotion may be retried at a
    /// deterministic all-nonzero anchor; this is not an identity rejection.
    AnchorOnGuardWall {
        epoch: Arc<FreshTaskEpoch>,
        circuit: Arc<ExactTargetCircuit>,
        cleared: Arc<ClearedExactCircuit>,
        refinement: ExactGuardRefinement,
        guard_ordinal: usize,
    },
}
