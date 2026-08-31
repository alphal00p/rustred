//! Retained semantic candidates and their exact point-routing graph.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::ExactTargetCircuit;
use crate::foundry::completion::guard::CoefficientIdealGuardAtom;
use crate::foundry::completion::guard::decision::{
    CoefficientIdealGuardDag, GuardDecisionEvaluationLimits, GuardDecisionOutcome,
};

use super::error::ExactCircuitSemanticError;

/// Canonical exact-content rank assigned only after a total structural sort.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct ExactCircuitSemanticCandidateId(pub(super) usize);

impl ExactCircuitSemanticCandidateId {
    pub(crate) const fn ordinal(self) -> usize {
        self.0
    }
}

/// One retained exact circuit and the semantic atoms compiled from all of its
/// nonzero guards. The retained `Arc` is the replay linkage returned on selection.
#[derive(Debug)]
pub(crate) struct ExactCircuitSemanticCandidate {
    pub(super) id: ExactCircuitSemanticCandidateId,
    pub(super) circuit: Arc<ExactTargetCircuit>,
    pub(super) guard_atoms: Box<[CoefficientIdealGuardAtom]>,
}

impl ExactCircuitSemanticCandidate {
    pub(crate) const fn id(&self) -> ExactCircuitSemanticCandidateId {
        self.id
    }

    pub(crate) const fn circuit(&self) -> &Arc<ExactTargetCircuit> {
        &self.circuit
    }

    pub(crate) fn guard_atoms(&self) -> &[CoefficientIdealGuardAtom] {
        &self.guard_atoms
    }
}

/// Result of exact point routing. `Incomplete` is a typed discovery gap and
/// grants no owner, RuleCell, negative relation, or closure authority.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ExactCircuitSemanticSelection<'a> {
    Selected(&'a ExactCircuitSemanticCandidate),
    Incomplete,
}

/// One target-partition-bound semantic decision graph over exact circuits.
#[derive(Debug)]
pub(crate) struct ExactCircuitSemanticDag {
    pub(super) context_fingerprint: Arc<String>,
    pub(super) candidates: Box<[ExactCircuitSemanticCandidate]>,
    pub(super) guards: CoefficientIdealGuardDag,
}

impl ExactCircuitSemanticDag {
    pub(crate) fn candidates(&self) -> &[ExactCircuitSemanticCandidate] {
        &self.candidates
    }

    pub(crate) fn guard_dag(&self) -> &CoefficientIdealGuardDag {
        &self.guards
    }

    /// Select at one complete exact integer assignment in the generic context.
    /// The selected borrow immutably identifies the replayed circuit and its
    /// compiled semantic evidence, but says nothing about a physical fibre.
    pub(crate) fn try_select_at(
        &self,
        context: &IndexedCoefficientContext,
        assignment: &[i64],
        limits: GuardDecisionEvaluationLimits,
    ) -> Result<ExactCircuitSemanticSelection<'_>, ExactCircuitSemanticError> {
        if context.fingerprint() != self.context_fingerprint.as_str() {
            return Err(ExactCircuitSemanticError::WrongContext);
        }
        match self
            .guards
            .try_decide_at(context, assignment, limits)
            .map_err(ExactCircuitSemanticError::GuardDag)?
        {
            GuardDecisionOutcome::Candidate(id) => self
                .candidates
                .get(id.0)
                .map(ExactCircuitSemanticSelection::Selected)
                .ok_or(ExactCircuitSemanticError::Invariant(
                    "guard DAG selected an absent exact candidate",
                )),
            GuardDecisionOutcome::Incomplete => Ok(ExactCircuitSemanticSelection::Incomplete),
        }
    }
}
