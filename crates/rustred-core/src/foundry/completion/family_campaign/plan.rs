use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use crate::family::IntegralKey;
use crate::sector::Mask;
use crate::sector::symmetry::Canonicalizer;

use super::{
    CompletePhysicalContractionGoal, CompletePhysicalContractionPlan, FamilyCoverageError,
    FamilyCoverageLimits, RequiredSectorOrbit,
};

pub(super) fn try_plan_complete_downset(
    goal: &CompletePhysicalContractionGoal,
    canonicalizer: &Canonicalizer,
    limits: FamilyCoverageLimits,
) -> Result<CompletePhysicalContractionPlan, FamilyCoverageError> {
    validate_canonicalizer(goal, canonicalizer)?;
    let raw_sector_count = checked_downset_size(goal.physical_slot_count())?;
    check_limit(
        "physical contraction masks",
        raw_sector_count,
        limits.max_physical_contractions,
    )?;

    let arity = goal.maximal_sector().arity();
    let mut physical_slots = Vec::new();
    physical_slots
        .try_reserve_exact(goal.physical_slot_count())
        .map_err(|_| FamilyCoverageError::AllocationFailure {
            resource: "physical slot ordinals",
            requested: goal.physical_slot_count(),
        })?;
    physical_slots.extend(
        goal.maximal_sector()
            .active_bits()
            .iter()
            .enumerate()
            .filter_map(|(slot, &active)| active.then_some(slot)),
    );

    let mut powers = Vec::new();
    powers
        .try_reserve_exact(arity)
        .map_err(|_| FamilyCoverageError::AllocationFailure {
            resource: "physical contraction powers",
            requested: arity,
        })?;
    powers.resize(arity, 0_i64);

    let mut canonical_counts = BTreeMap::<IntegralKey, usize>::new();
    for ordinal in 0..raw_sector_count {
        for (bit, &slot) in physical_slots.iter().enumerate() {
            powers[slot] = i64::from(((ordinal >> bit) & 1) != 0);
        }
        let raw = IntegralKey::try_new(powers.iter().copied())?;
        let canonical = canonicalizer.canonicalize(&raw)?.canonical().clone();
        let requested = canonical_counts.len().checked_add(1).ok_or(
            FamilyCoverageError::ResourceCountOverflow {
                resource: "canonical sector orbit count",
            },
        )?;
        match canonical_counts.entry(canonical) {
            Entry::Vacant(entry) => {
                check_limit(
                    "canonical sector orbits",
                    requested,
                    limits.max_sector_orbits,
                )?;
                entry.insert(1);
            }
            Entry::Occupied(mut entry) => {
                let count = entry.get_mut();
                *count =
                    count
                        .checked_add(1)
                        .ok_or(FamilyCoverageError::ResourceCountOverflow {
                            resource: "raw sectors in one canonical orbit",
                        })?;
            }
        }
    }

    let mut required_orbits = Vec::new();
    required_orbits
        .try_reserve_exact(canonical_counts.len())
        .map_err(|_| FamilyCoverageError::AllocationFailure {
            resource: "required sector orbits",
            requested: canonical_counts.len(),
        })?;
    for (corner, raw_sector_count) in canonical_counts {
        let sector = Mask::try_from_indices(corner.powers())?;
        required_orbits.push(RequiredSectorOrbit::new(sector, corner, raw_sector_count));
    }
    // Staged publication consumes same-active-count waves. Keep those waves
    // contiguous while retaining lexical corner order within each wave.
    required_orbits.sort_unstable_by(|left, right| {
        left.sector()
            .active_count()
            .cmp(&right.sector().active_count())
            .then_with(|| left.corner().cmp(right.corner()))
    });

    Ok(CompletePhysicalContractionPlan::new(
        goal.family_fingerprint_owner(),
        goal.maximal_sector().clone(),
        raw_sector_count,
        required_orbits,
    ))
}

fn validate_canonicalizer(
    goal: &CompletePhysicalContractionGoal,
    canonicalizer: &Canonicalizer,
) -> Result<(), FamilyCoverageError> {
    if canonicalizer.family_fingerprint() != goal.family_fingerprint() {
        return Err(FamilyCoverageError::WrongCanonicalizerFamily);
    }
    let expected = goal.maximal_sector().arity();
    let actual = canonicalizer.arity();
    if actual != expected {
        return Err(FamilyCoverageError::WrongCanonicalizerArity { expected, actual });
    }
    for (group_element, source_for_target) in canonicalizer.group_elements().enumerate() {
        for (target_slot, &source_slot) in source_for_target.iter().enumerate() {
            if goal.maximal_sector().active_bits()[target_slot]
                != goal.maximal_sector().active_bits()[source_slot]
            {
                return Err(FamilyCoverageError::SlotRolesNotSymmetryInvariant {
                    group_element,
                    target_slot,
                    source_slot,
                });
            }
        }
    }
    Ok(())
}

pub(super) fn checked_downset_size(
    physical_slot_count: usize,
) -> Result<usize, FamilyCoverageError> {
    let shift = u32::try_from(physical_slot_count).map_err(|_| {
        FamilyCoverageError::PhysicalContractionCountOverflow {
            physical_slot_count,
        }
    })?;
    1_usize
        .checked_shl(shift)
        .ok_or(FamilyCoverageError::PhysicalContractionCountOverflow {
            physical_slot_count,
        })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FamilyCoverageError> {
    if requested > limit {
        Err(FamilyCoverageError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}
