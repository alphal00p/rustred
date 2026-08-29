use crate::identity::IndexShift;
use crate::sector::{Mask, OrderingPolicy, SectorMonotoneDomain};

use super::super::error::ParametricRuleError;
use super::super::limits::ParametricRuleLimits;
use super::super::model::ParametricRuleTerm;
use super::super::prepare::{check_limit, checked_add, try_vec};
use super::model::{SectorMonotoneDependency, SectorMonotoneTargetAdmission};

/// An RHS shift may deactivate active lines, but it may never activate an
/// inactive parent line without a different piecewise-domain proof.
pub(crate) fn preflight_sector_monotone_rhs_shift(
    parent_sector: &Mask,
    right_hand_side_ordinal: usize,
    shift: &IndexShift,
) -> Result<(), ParametricRuleError> {
    if shift.values().len() != parent_sector.arity() {
        return Err(ParametricRuleError::ReducerInvariant {
            detail: "sector-monotone RHS shift does not match the parent-sector arity",
        });
    }
    for (position, (&active, &shift)) in parent_sector
        .active_bits()
        .iter()
        .zip(shift.values())
        .enumerate()
    {
        if !active && shift > 0 {
            return Err(ParametricRuleError::ActivationLeakRequiresRefinement {
                right_hand_side_ordinal,
                position,
                shift,
            });
        }
    }
    Ok(())
}

pub(crate) fn build_sector_monotone_admission(
    parent_sector: &Mask,
    pivot: &IndexShift,
    right_hand_side: &[ParametricRuleTerm],
    ordering: OrderingPolicy,
    limits: ParametricRuleLimits,
) -> Result<SectorMonotoneTargetAdmission, ParametricRuleError> {
    let mut shift_slices = try_vec("sector-monotone RHS shift views", right_hand_side.len())?;
    shift_slices.extend(right_hand_side.iter().map(|term| term.shift().values()));
    let domain = SectorMonotoneDomain::try_maximal_for_rule(
        parent_sector.clone(),
        pivot.values(),
        &shift_slices,
    )?;

    preflight_retained_thresholds(
        &domain,
        &shift_slices,
        limits.max_sector_monotone_thresholds,
    )?;

    let mut dependencies = try_vec(
        "sector-monotone universal dependencies",
        right_hand_side.len(),
    )?;
    for (right_hand_side_ordinal, term) in right_hand_side.iter().enumerate() {
        let proof = ordering
            .prove_sector_monotone_shift_descent(&domain, pivot.values(), term.shift().values())
            .map_err(|error| match error {
                crate::sector::Error::InactiveLineActivation { position, shift } => {
                    ParametricRuleError::ActivationLeakRequiresRefinement {
                        right_hand_side_ordinal,
                        position,
                        shift,
                    }
                }
                crate::sector::Error::NotStrictDescent => {
                    ParametricRuleError::SectorMonotoneTermNotDescending {
                        right_hand_side_ordinal,
                    }
                }
                error => ParametricRuleError::Ordering(error),
            })?;
        dependencies.push(SectorMonotoneDependency::new(
            right_hand_side_ordinal,
            pivot.clone(),
            term.shift().clone(),
            proof,
        ));
    }
    let admission = SectorMonotoneTargetAdmission::new(domain, pivot.clone(), dependencies);
    if !admission.verify() {
        return Err(ParametricRuleError::ReducerInvariant {
            detail: "constructed sector-monotone universal admission failed self-verification",
        });
    }
    Ok(admission)
}

fn preflight_retained_thresholds(
    domain: &SectorMonotoneDomain,
    right_hand_side_shifts: &[&[i64]],
    limit: usize,
) -> Result<(), ParametricRuleError> {
    // Reuse the sector owner's exact compact-partition census before any term
    // proof allocates its threshold buffer.
    let mut threshold_count = 0usize;
    for shift in right_hand_side_shifts {
        threshold_count = checked_add(
            "sector-monotone active pinch thresholds",
            threshold_count,
            domain.retained_pinch_threshold_count(shift)?,
        )?;
    }
    check_limit(
        "sector-monotone active pinch thresholds",
        threshold_count,
        limit,
    )
}

#[cfg(test)]
mod tests {
    use crate::sector::{Mask, SectorMonotoneDomain};

    use super::{ParametricRuleError, preflight_retained_thresholds};

    #[test]
    fn threshold_preflight_stops_after_an_everywhere_pinched_coordinate() {
        let domain = SectorMonotoneDomain::try_maximal_for_rule(
            Mask::try_new([true, true]).unwrap(),
            &[0, 0],
            &[[i64::MIN, -1]],
        )
        .unwrap();
        let shift = [i64::MIN, -1];
        let shifts = [&shift[..]];
        preflight_retained_thresholds(&domain, &shifts, 1).unwrap();
        assert_eq!(
            preflight_retained_thresholds(&domain, &shifts, 0),
            Err(ParametricRuleError::ResourceLimit {
                resource: "sector-monotone active pinch thresholds",
                requested: 1,
                limit: 0,
            })
        );
    }
}
