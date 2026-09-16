//! Conservative no-new-index-poles admission for a compact certificate.
//!
//! Native exception extraction/intersection owns the algebra. Only existing
//! exact excluded cases or coordinate faces disjoint from the supplied boxes
//! are admitted here; unsupported geometry selects the old exact fallback.

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::foundry::completion::LatticeBox;
use crate::solver::{RuleCandidate, SectorRule, Term, extract_exceptions};

use super::{SourcePortAuditError, error};

pub(super) fn conditions_are_index_free<const N: usize>(
    conditions: &[CoefficientPolynomial],
    indices: &[usize; N],
) -> bool {
    conditions.iter().all(|condition| {
        indices
            .iter()
            .all(|&axis| axis < condition.nvars() && !condition.contains(axis))
    })
}

pub(super) fn preserves_domain<const N: usize>(
    weights: &[Coefficient],
    rule: &SectorRule<N>,
    boxes: &[LatticeBox],
    indices: &[usize; N],
    sector: &[bool; N],
) -> Result<bool, SourcePortAuditError> {
    let candidate = RuleCandidate {
        case: rule.candidate.case.clone(),
        target: rule.candidate.target,
        rhs: weights
            .iter()
            .filter(|weight| !weight.is_zero())
            .map(|weight| Term {
                integral: rule.candidate.target,
                coefficient: weight.clone(),
            })
            .collect(),
        sources: Vec::new(),
        stats: Default::default(),
    };
    let exceptions = extract_exceptions(&candidate, indices, sector).map_err(error)?;
    if exceptions.branches.is_empty() {
        return Ok(true);
    }
    let exceptional = SectorRule {
        candidate,
        exceptions,
    }
    .exceptional_cases(indices, sector)
    .map_err(error)?;
    let existing = rule.exceptional_cases(indices, sector).map_err(error)?;
    for case in exceptional {
        if existing.contains(&case) {
            continue;
        }
        let Some(face) = case.coordinate() else {
            return Ok(false);
        };
        if boxes.iter().any(|piece| {
            // A coordinate face meets the box iff all fixed coordinates do.
            // Unfixed coordinates impose no restrictions. Affine targets are
            // deliberately not approximated: these boxes are supersets.
            if piece.arity() != N {
                return true;
            }
            face.fixed().iter().enumerate().all(|(axis, fixed)| {
                let Some(fixed) = fixed else {
                    return true;
                };
                let local = if sector[axis] {
                    i64::from(*fixed) - 1
                } else {
                    -i64::from(*fixed)
                };
                let Ok(local) = u64::try_from(local) else {
                    return false;
                };
                local >= piece.lower()[axis]
                    && piece.upper()[axis].is_none_or(|upper| local <= upper)
            })
        }) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
