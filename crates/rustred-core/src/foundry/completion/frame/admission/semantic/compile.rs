//! Deterministic compilation of replayed circuits into a semantic guard DAG.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::ExactTargetCircuit;
use crate::foundry::completion::guard::CoefficientIdealGuardAtom;
use crate::foundry::completion::guard::decision::{
    CoefficientIdealGuardDag, GuardDecisionCandidate, GuardDecisionCandidateId,
};
use crate::foundry::completion::stratum::TargetColumnPartition;

use super::error::ExactCircuitSemanticError;
use super::limits::ExactCircuitSemanticLimits;
use super::model::{
    ExactCircuitSemanticCandidate, ExactCircuitSemanticCandidateId, ExactCircuitSemanticDag,
};
use super::order::compare_exact_circuit_content;
use super::validation::{
    CANDIDATES, ContentTotals, NONZERO_GUARDS, charge_compiled_guard_atom, check_limit, try_vec,
    validate_candidate, validate_partition,
};

impl ExactCircuitSemanticDag {
    pub(crate) fn try_compile(
        context: &IndexedCoefficientContext,
        partition: &TargetColumnPartition<'_>,
        incoming: &[Arc<ExactTargetCircuit>],
        limits: ExactCircuitSemanticLimits,
    ) -> Result<Self, ExactCircuitSemanticError> {
        validate_partition(context, partition)?;
        check_limit(CANDIDATES, incoming.len(), limits.max_candidates)?;
        check_limit(CANDIDATES, incoming.len(), limits.guard_dag.max_candidates)?;

        let mut totals = ContentTotals::default();
        for (candidate, circuit) in incoming.iter().enumerate() {
            validate_candidate(context, partition, circuit, candidate, limits, &mut totals)?;
        }

        let mut ordered = try_vec(CANDIDATES, incoming.len())?;
        ordered.extend(incoming.iter().cloned());
        ordered.sort_unstable_by(|left, right| compare_exact_circuit_content(left, right));
        if ordered
            .windows(2)
            .any(|pair| compare_exact_circuit_content(&pair[0], &pair[1]) == Ordering::Equal)
        {
            return Err(ExactCircuitSemanticError::DuplicateExactContent);
        }

        let mut candidates = try_vec(CANDIDATES, ordered.len())?;
        for (ordinal, circuit) in ordered.into_iter().enumerate() {
            let mut atoms = try_vec(NONZERO_GUARDS, circuit.nonzero_guards().len())?;
            for (guard_ordinal, guard) in circuit.nonzero_guards().iter().enumerate() {
                let atom = CoefficientIdealGuardAtom::try_for_target(
                    context,
                    guard.polynomial(),
                    circuit.target_shift().values(),
                    limits.guard_atom,
                )
                .map_err(|error| ExactCircuitSemanticError::GuardAtom {
                    candidate: ordinal,
                    guard: guard_ordinal,
                    error,
                })?;
                charge_compiled_guard_atom(&atom, limits, &mut totals)?;
                atoms.push(atom);
            }
            candidates.push(ExactCircuitSemanticCandidate {
                id: ExactCircuitSemanticCandidateId(ordinal),
                circuit,
                guard_atoms: atoms.into_boxed_slice(),
            });
        }

        let guards = {
            let mut borrowed = try_vec(CANDIDATES, candidates.len())?;
            for candidate in &candidates {
                borrowed.push(GuardDecisionCandidate::new(
                    GuardDecisionCandidateId(candidate.id.0),
                    candidate.guard_atoms(),
                ));
            }
            CoefficientIdealGuardDag::try_compile(context, &borrowed, limits.guard_dag)
                .map_err(ExactCircuitSemanticError::GuardDag)?
        };
        Ok(Self {
            plan_identity: partition.frame().identity_owner(),
            context_fingerprint: context.fingerprint_owner(),
            candidates: candidates.into_boxed_slice(),
            guards,
        })
    }
}
