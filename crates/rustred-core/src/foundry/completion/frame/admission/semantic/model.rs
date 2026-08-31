//! Retained semantic candidates and their exact point-routing graph.

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::ExactTargetCircuit;
use crate::foundry::completion::frame::{PhysicalFramePlan, PhysicalFramePlanIdentity};
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
    pub(super) plan_identity: PhysicalFramePlanIdentity,
    pub(super) context_fingerprint: Arc<String>,
    pub(super) candidates: Box<[ExactCircuitSemanticCandidate]>,
    pub(super) guards: CoefficientIdealGuardDag,
}

impl ExactCircuitSemanticDag {
    pub(crate) fn is_bound_to(&self, plan: &PhysicalFramePlan) -> bool {
        self.plan_identity.belongs_to(plan)
    }

    pub(crate) fn context_fingerprint(&self) -> &str {
        self.context_fingerprint.as_str()
    }

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

#[cfg(test)]
impl ExactCircuitSemanticDag {
    /// Unit-test seam for exercising conservative owner-cover behavior with
    /// additional sound restrictions on already replayed exact circuits.
    pub(crate) fn try_from_test_candidates(
        context: &IndexedCoefficientContext,
        baseline: &Self,
        incoming: Vec<(Arc<ExactTargetCircuit>, Vec<CoefficientIdealGuardAtom>)>,
    ) -> Result<Self, ExactCircuitSemanticError> {
        use crate::foundry::completion::guard::decision::{
            GuardDecisionCandidate, GuardDecisionCandidateId,
        };

        let candidates = incoming
            .into_iter()
            .enumerate()
            .map(
                |(ordinal, (circuit, guard_atoms))| ExactCircuitSemanticCandidate {
                    id: ExactCircuitSemanticCandidateId(ordinal),
                    circuit,
                    guard_atoms: guard_atoms.into_boxed_slice(),
                },
            )
            .collect::<Vec<_>>();
        let borrowed = candidates
            .iter()
            .map(|candidate| {
                GuardDecisionCandidate::new(
                    GuardDecisionCandidateId(candidate.id.0),
                    candidate.guard_atoms(),
                )
            })
            .collect::<Vec<_>>();
        if baseline.context_fingerprint() != context.fingerprint() {
            return Err(ExactCircuitSemanticError::WrongContext);
        }
        let guards = CoefficientIdealGuardDag::try_compile(context, &borrowed, Default::default())
            .map_err(ExactCircuitSemanticError::GuardDag)?;
        Ok(Self {
            plan_identity: baseline.plan_identity.clone(),
            context_fingerprint: context.fingerprint_owner(),
            candidates: candidates.into_boxed_slice(),
            guards,
        })
    }
}
