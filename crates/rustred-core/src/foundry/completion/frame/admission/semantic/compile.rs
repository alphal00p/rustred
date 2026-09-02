//! Deterministic compilation of replayed circuits into a semantic guard DAG.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::completion::frame::exact::{ClearedExactCircuit, ExactTargetCircuit};
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

struct SemanticInput {
    circuit: Arc<ExactTargetCircuit>,
    cleared: Option<Arc<ClearedExactCircuit>>,
}

impl ExactCircuitSemanticDag {
    pub(crate) fn try_compile(
        context: &IndexedCoefficientContext,
        partition: &TargetColumnPartition<'_>,
        incoming: &[Arc<ExactTargetCircuit>],
        limits: ExactCircuitSemanticLimits,
    ) -> Result<Self, ExactCircuitSemanticError> {
        let mut inputs = try_vec(CANDIDATES, incoming.len())?;
        inputs.extend(incoming.iter().cloned().map(|circuit| SemanticInput {
            circuit,
            cleared: None,
        }));
        Self::try_compile_inputs(context, partition, inputs, limits)
    }

    /// Compile semantic routing from fraction-free source consequences rather
    /// than from one Gaussian elimination path's guard chronology.
    pub(crate) fn try_compile_cleared(
        context: &IndexedCoefficientContext,
        partition: &TargetColumnPartition<'_>,
        incoming: &[(Arc<ExactTargetCircuit>, Arc<ClearedExactCircuit>)],
        limits: ExactCircuitSemanticLimits,
    ) -> Result<Self, ExactCircuitSemanticError> {
        let mut inputs = try_vec(CANDIDATES, incoming.len())?;
        for (candidate, (circuit, cleared)) in incoming.iter().enumerate() {
            if !cleared.is_bound_to(circuit) {
                return Err(ExactCircuitSemanticError::CandidateJoin {
                    candidate,
                    detail: "fraction-free guard certificate belongs to another exact circuit",
                });
            }
            inputs.push(SemanticInput {
                circuit: circuit.clone(),
                cleared: Some(cleared.clone()),
            });
        }
        Self::try_compile_inputs(context, partition, inputs, limits)
    }

    fn try_compile_inputs(
        context: &IndexedCoefficientContext,
        partition: &TargetColumnPartition<'_>,
        mut incoming: Vec<SemanticInput>,
        limits: ExactCircuitSemanticLimits,
    ) -> Result<Self, ExactCircuitSemanticError> {
        validate_partition(context, partition)?;
        check_limit(CANDIDATES, incoming.len(), limits.max_candidates)?;
        check_limit(CANDIDATES, incoming.len(), limits.guard_dag.max_candidates)?;

        let mut totals = ContentTotals::default();
        for (candidate, input) in incoming.iter().enumerate() {
            validate_candidate(
                context,
                partition,
                &input.circuit,
                candidate,
                limits,
                &mut totals,
            )?;
            if let Some(cleared) = &input.cleared {
                check_limit(
                    NONZERO_GUARDS,
                    cleared.semantic_guards().len(),
                    limits.max_nonzero_guards,
                )?;
            }
        }

        incoming.sort_unstable_by(|left, right| {
            compare_exact_circuit_content(&left.circuit, &right.circuit)
        });
        if incoming.windows(2).any(|pair| {
            compare_exact_circuit_content(&pair[0].circuit, &pair[1].circuit) == Ordering::Equal
        }) {
            return Err(ExactCircuitSemanticError::DuplicateExactContent);
        }

        let mut candidates = try_vec(CANDIDATES, incoming.len())?;
        for (ordinal, input) in incoming.into_iter().enumerate() {
            let circuit = input.circuit;
            let guard_count = input.cleared.as_ref().map_or_else(
                || circuit.nonzero_guards().len(),
                |cleared| cleared.semantic_guards().len(),
            );
            let mut atoms = try_vec(NONZERO_GUARDS, guard_count)?;
            if let Some(cleared) = input.cleared {
                for (guard_ordinal, guard) in cleared.semantic_guards().iter().enumerate() {
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
            } else {
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
