use super::super::super::model::CoefficientIdealGuardPredicate;
use super::super::model::CanonicalGuardCandidate;
use super::super::{GuardDecisionCandidate, GuardDecisionDagError, GuardDecisionDagLimits};
use super::resource::{
    ATOM_IDENTITY_BYTES, CANDIDATE_ATOM_REFERENCES, CANDIDATES, UNIQUE_ATOMS, check_limit,
    checked_add, try_clone_boxed, try_vec,
};

pub(super) fn validate(
    context_fingerprint: &str,
    atoms: &[CoefficientIdealGuardPredicate],
    candidates: &[CanonicalGuardCandidate],
    limits: GuardDecisionDagLimits,
) -> Result<(), GuardDecisionDagError> {
    check_limit(
        "semantic guard DAG context identity bytes",
        context_fingerprint.len(),
        limits.max_context_identity_bytes,
    )?;
    check_limit(CANDIDATES, candidates.len(), limits.max_candidates)?;
    check_limit(UNIQUE_ATOMS, atoms.len(), limits.max_unique_atoms)?;
    if atoms.windows(2).any(|pair| pair[0].id() >= pair[1].id()) {
        return Err(GuardDecisionDagError::InternalInvariant(
            "global guard atoms are not strictly ordered",
        ));
    }
    check_atom_identity_bytes(atoms.iter(), limits)?;
    let mut references = 0usize;
    require_canonical_ids(candidates.iter().map(|candidate| candidate.id))?;
    for candidate in candidates {
        if candidate
            .required_atoms
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(GuardDecisionDagError::InternalInvariant(
                "candidate guard atoms are not strictly ordered",
            ));
        }
        if candidate
            .required_atoms
            .last()
            .is_some_and(|&atom| atom >= atoms.len())
        {
            return Err(GuardDecisionDagError::InternalInvariant(
                "candidate guard atom is out of range",
            ));
        }
        references = checked_add(
            CANDIDATE_ATOM_REFERENCES,
            references,
            candidate.required_atoms.len(),
        )?;
    }
    check_limit(
        CANDIDATE_ATOM_REFERENCES,
        references,
        limits.max_candidate_atom_references,
    )
}

pub(super) fn require_canonical_candidate_ids(
    candidates: &[GuardDecisionCandidate<'_>],
) -> Result<(), GuardDecisionDagError> {
    require_canonical_ids(candidates.iter().map(|candidate| candidate.id()))
}

fn require_canonical_ids(
    ids: impl IntoIterator<Item = super::super::GuardDecisionCandidateId>,
) -> Result<(), GuardDecisionDagError> {
    let mut previous = None;
    for id in ids {
        if let Some(previous) = previous {
            if id == previous {
                return Err(GuardDecisionDagError::DuplicateCandidate { candidate: id.0 });
            }
            if id < previous {
                return Err(GuardDecisionDagError::NonCanonicalCandidateOrder {
                    previous: previous.0,
                    current: id.0,
                });
            }
        }
        previous = Some(id);
    }
    Ok(())
}

pub(super) fn check_atom_identity_bytes<'a>(
    atoms: impl IntoIterator<Item = &'a CoefficientIdealGuardPredicate>,
    limits: GuardDecisionDagLimits,
) -> Result<(), GuardDecisionDagError> {
    let bytes = atoms.into_iter().try_fold(0usize, |total, atom| {
        let total = checked_add(
            ATOM_IDENTITY_BYTES,
            total,
            atom.representative_identity().predicate().len(),
        )?;
        atom.id()
            .generators()
            .iter()
            .try_fold(total, |total, generator| {
                checked_add(ATOM_IDENTITY_BYTES, total, generator.predicate().len())
            })
    })?;
    check_limit(ATOM_IDENTITY_BYTES, bytes, limits.max_atom_identity_bytes)
}

pub(super) fn try_clone_candidates(
    candidates: &[CanonicalGuardCandidate],
) -> Result<Box<[CanonicalGuardCandidate]>, GuardDecisionDagError> {
    let mut retained = try_vec(candidates.len(), CANDIDATES)?;
    for candidate in candidates {
        retained.push(CanonicalGuardCandidate {
            id: candidate.id,
            required_atoms: try_clone_boxed(&candidate.required_atoms, CANDIDATE_ATOM_REFERENCES)?,
        });
    }
    Ok(retained.into_boxed_slice())
}
