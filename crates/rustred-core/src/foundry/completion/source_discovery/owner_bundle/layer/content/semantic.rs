use crate::foundry::completion::frame::admission::{ExactCircuitSemanticDag, ExactGuardRefinement};
use crate::foundry::completion::guard::CoefficientIdealGuardAtom;
use crate::foundry::completion::stratum::StratumRegistryError;

use super::algebra::{append_decorated_stratum, append_guard_branch_identity, append_polynomial};
use super::encoder::BoundedContentHasher;
use super::exact::append_exact_circuit;

pub(super) fn append_semantic_dag(
    output: &mut BoundedContentHasher,
    semantic: &ExactCircuitSemanticDag,
) -> Result<(), StratumRegistryError> {
    output.text(semantic.context_fingerprint())?;
    output.count(semantic.candidates().len())?;
    for candidate in semantic.candidates() {
        output.usize(candidate.id().ordinal())?;
        append_exact_circuit(output, candidate.circuit())?;
        output.count(candidate.guard_atoms().len())?;
        for atom in candidate.guard_atoms() {
            append_guard_atom(output, atom)?;
        }
    }

    // The verified decision DAG is a canonical compilation of this ordered
    // candidate/atom sequence. Its derived node allocation is not a separate
    // mathematical choice; these stats pin the retained executable shape and
    // detect accidental compiler drift without importing private node types.
    let stats = semantic.guard_dag().stats();
    output.usize(stats.candidates)?;
    output.usize(stats.atoms)?;
    output.usize(stats.candidate_atom_references)?;
    output.usize(stats.memo_states)?;
    output.usize(stats.nodes)?;
    output.usize(stats.edges)?;
    output.boolean(stats.has_reachable_incomplete)
}

fn append_guard_atom(
    output: &mut BoundedContentHasher,
    atom: &CoefficientIdealGuardAtom,
) -> Result<(), StratumRegistryError> {
    output.text(atom.context_fingerprint())?;
    output.count(atom.id().generators().len())?;
    for generator in atom.id().generators() {
        append_guard_branch_identity(output, generator)?;
    }
    append_guard_branch_identity(output, atom.predicate().representative_identity())?;
    append_polynomial(output, atom.predicate().representative_guard())?;
    output.count(atom.coefficient_system().equations().len())?;
    for equation in atom.coefficient_system().equations() {
        output.count(equation.base_monomial().len())?;
        for &power in equation.base_monomial() {
            output.u16(power)?;
        }
        append_polynomial(output, equation.index_polynomial())?;
    }
    output.boolean(atom.has_literal_unit_generator())
}

pub(super) fn append_guard_refinement(
    output: &mut BoundedContentHasher,
    refinement: &ExactGuardRefinement,
) -> Result<(), StratumRegistryError> {
    output.text(refinement.parent_stratum_id().as_str())?;
    output.count(refinement.required_predicates().len())?;
    for required in refinement.required_predicates() {
        append_guard_branch_identity(output, required.nonzero_branch())?;
        output.count(required.circuit_guard_ordinals().len())?;
        for &ordinal in required.circuit_guard_ordinals() {
            output.usize(ordinal)?;
        }
    }
    output.count(refinement.newly_split_predicate_ordinals().len())?;
    for &ordinal in refinement.newly_split_predicate_ordinals() {
        output.usize(ordinal)?;
    }
    append_decorated_stratum(output, refinement.admitted_stratum())?;
    output.count(refinement.exceptional_strata().len())?;
    for exceptional in refinement.exceptional_strata() {
        output.usize(exceptional.required_predicate_ordinal())?;
        append_decorated_stratum(output, exceptional.stratum())?;
    }
    Ok(())
}
