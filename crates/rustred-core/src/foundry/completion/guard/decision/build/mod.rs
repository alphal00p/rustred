mod canonical;
mod dag;
mod resource;

use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;

use super::super::model::CoefficientIdealGuardPredicate;
use super::model::CanonicalGuardCandidate;
use super::{
    CoefficientIdealGuardDag, GuardDecisionCandidate, GuardDecisionDagError,
    GuardDecisionDagLimits, GuardDecisionDagStats,
};
use canonical::{
    check_atom_identity_bytes, require_canonical_candidate_ids, try_clone_candidates, validate,
};
use resource::{
    CANDIDATE_ATOM_REFERENCES, CANDIDATES, EDGES, NODES, UNIQUE_ATOMS, check_limit, checked_add,
    try_clone_boxed, try_vec,
};

impl CoefficientIdealGuardDag {
    pub(crate) fn try_compile(
        context: &IndexedCoefficientContext,
        candidates: &[GuardDecisionCandidate<'_>],
        limits: GuardDecisionDagLimits,
    ) -> Result<Self, GuardDecisionDagError> {
        check_limit(CANDIDATES, candidates.len(), limits.max_candidates)?;
        check_limit(
            "semantic guard DAG context identity bytes",
            context.fingerprint().len(),
            limits.max_context_identity_bytes,
        )?;
        require_canonical_candidate_ids(candidates)?;

        let raw_references = candidates.iter().try_fold(0usize, |total, candidate| {
            checked_add(
                CANDIDATE_ATOM_REFERENCES,
                total,
                candidate.required_nonzero().len(),
            )
        })?;
        check_limit(
            CANDIDATE_ATOM_REFERENCES,
            raw_references,
            limits.max_candidate_atom_references,
        )?;

        let mut raw_atoms = try_vec(raw_references, UNIQUE_ATOMS)?;
        for candidate in candidates {
            for (atom_ordinal, atom) in candidate.required_nonzero().iter().enumerate() {
                if atom.context_fingerprint() != context.fingerprint() {
                    return Err(GuardDecisionDagError::WrongAtomContext {
                        candidate: candidate.id().0,
                        atom: atom_ordinal,
                    });
                }
                if !atom.has_literal_unit_generator() {
                    raw_atoms.push(atom.predicate());
                }
            }
        }
        raw_atoms.sort_unstable_by(|left, right| {
            left.id().cmp(right.id()).then_with(|| {
                left.representative_identity()
                    .cmp(right.representative_identity())
            })
        });
        let mut representatives = try_vec(raw_atoms.len(), UNIQUE_ATOMS)?;
        for atom in raw_atoms {
            if representatives
                .last()
                .is_none_or(|previous: &&CoefficientIdealGuardPredicate| previous.id() != atom.id())
            {
                representatives.push(atom);
            }
        }
        check_limit(UNIQUE_ATOMS, representatives.len(), limits.max_unique_atoms)?;
        check_atom_identity_bytes(representatives.iter().copied(), limits)?;
        let mut atoms = try_vec(representatives.len(), UNIQUE_ATOMS)?;
        atoms.extend(representatives.into_iter().cloned());

        let mut canonical = try_vec(candidates.len(), CANDIDATES)?;
        for candidate in candidates {
            let mut required = try_vec(
                candidate.required_nonzero().len(),
                CANDIDATE_ATOM_REFERENCES,
            )?;
            for atom in candidate.required_nonzero() {
                if atom.has_literal_unit_generator() {
                    continue;
                }
                let ordinal = atoms
                    .binary_search_by(|candidate| candidate.id().cmp(atom.id()))
                    .map_err(|_| {
                        GuardDecisionDagError::InternalInvariant(
                            "canonical guard atom is absent from the global order",
                        )
                    })?;
                required.push(ordinal);
            }
            required.sort_unstable();
            required.dedup();
            canonical.push(CanonicalGuardCandidate {
                id: candidate.id(),
                required_atoms: required.into_boxed_slice(),
            });
        }

        compile_canonical(
            context.fingerprint_owner(),
            atoms.into_boxed_slice(),
            canonical.into_boxed_slice(),
            limits,
        )
    }
}

pub(super) fn try_verify(
    dag: &CoefficientIdealGuardDag,
    limits: GuardDecisionDagLimits,
) -> Result<bool, GuardDecisionDagError> {
    validate(
        dag.context_fingerprint(),
        &dag.atoms,
        &dag.candidates,
        limits,
    )?;
    let rebuilt = compile_canonical(
        dag.context_fingerprint.clone(),
        try_clone_boxed(&dag.atoms, UNIQUE_ATOMS)?,
        try_clone_candidates(&dag.candidates)?,
        limits,
    )?;
    Ok(rebuilt == *dag)
}

fn compile_canonical(
    context_fingerprint: Arc<String>,
    atoms: Box<[CoefficientIdealGuardPredicate]>,
    candidates: Box<[CanonicalGuardCandidate]>,
    limits: GuardDecisionDagLimits,
) -> Result<CoefficientIdealGuardDag, GuardDecisionDagError> {
    validate(&context_fingerprint, &atoms, &candidates, limits)?;
    let built = dag::try_build(&atoms, &candidates, limits)?;
    let candidate_atom_references = candidates.iter().try_fold(0usize, |total, candidate| {
        checked_add(
            CANDIDATE_ATOM_REFERENCES,
            total,
            candidate.required_atoms.len(),
        )
    })?;
    let edges = built
        .nodes
        .len()
        .checked_mul(2)
        .ok_or(GuardDecisionDagError::ResourceCountOverflow { resource: EDGES })?;
    let has_reachable_incomplete = reachable_incomplete(&built.nodes, built.root)?;
    let stats = GuardDecisionDagStats {
        candidates: candidates.len(),
        atoms: atoms.len(),
        candidate_atom_references,
        memo_states: built.memo_states,
        nodes: built.nodes.len(),
        edges,
        has_reachable_incomplete,
    };
    Ok(CoefficientIdealGuardDag {
        context_fingerprint,
        atoms: Arc::from(atoms),
        candidates: Arc::from(candidates),
        nodes: Arc::from(built.nodes),
        root: built.root,
        stats,
    })
}

fn reachable_incomplete(
    nodes: &[super::model::GuardDecisionNode],
    root: super::model::GuardDecisionRef,
) -> Result<bool, GuardDecisionDagError> {
    use super::GuardDecisionOutcome;
    use super::model::GuardDecisionRef;

    let mut incomplete = resource::try_vec(nodes.len(), NODES)?;
    for (ordinal, node) in nodes.iter().enumerate() {
        let child_is_incomplete = |child| match child {
            GuardDecisionRef::Leaf(GuardDecisionOutcome::Incomplete) => Ok(true),
            GuardDecisionRef::Leaf(GuardDecisionOutcome::Candidate(_)) => Ok(false),
            GuardDecisionRef::Node(child) => {
                if child >= ordinal {
                    return Err(GuardDecisionDagError::InternalInvariant(
                        "guard DAG child is not in postorder",
                    ));
                }
                incomplete
                    .get(child)
                    .copied()
                    .ok_or(GuardDecisionDagError::InternalInvariant(
                        "guard DAG child totality is unavailable",
                    ))
            }
        };
        incomplete.push(child_is_incomplete(node.zero)? || child_is_incomplete(node.nonzero)?);
    }
    match root {
        GuardDecisionRef::Leaf(GuardDecisionOutcome::Incomplete) => Ok(true),
        GuardDecisionRef::Leaf(GuardDecisionOutcome::Candidate(_)) => Ok(false),
        GuardDecisionRef::Node(root) => {
            incomplete
                .get(root)
                .copied()
                .ok_or(GuardDecisionDagError::InternalInvariant(
                    "guard DAG root totality is unavailable",
                ))
        }
    }
}
