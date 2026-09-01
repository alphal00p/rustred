use super::super::simplex_support::checked_binomial;
use super::BoundarySimplexPlanError;
use super::resource::try_reserve_exact;

/// Unrank one lexicographic c-subset of ascending ambient free axes.
///
/// No table of all subsets is retained. The caller preflights a conservative
/// work envelope; this routine allocates only the selected pinned and
/// remaining axes for one face. Subset unranking is boundary-specific; the
/// simplex and finite-assignment combinatorics are shared with the interior
/// planner.
pub(super) fn try_unrank_axis_subset(
    free_axes: &[usize],
    subset_size: usize,
    mut ordinal: usize,
) -> Result<(Vec<usize>, Vec<usize>), BoundarySimplexPlanError> {
    if subset_size > free_axes.len() {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "a boundary subset exceeded its parent free dimension",
        });
    }
    let expected = checked_binomial(free_axes.len(), subset_size, "boundary faces per parent")?;
    if ordinal >= expected {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "boundary subset ordinal exceeded its exact binomial count",
        });
    }

    let mut local_indices = Vec::new();
    try_reserve_exact(&mut local_indices, subset_size, "pinned-axis local indices")?;
    let mut next_candidate = 0usize;
    for slot in 0..subset_size {
        let remaining_slots = subset_size - slot - 1;
        let maximal_candidate = free_axes.len() - remaining_slots - 1;
        let mut selected = None;
        for candidate in next_candidate..=maximal_candidate {
            let suffix_count = checked_binomial(
                free_axes.len() - candidate - 1,
                remaining_slots,
                "boundary subset unranking",
            )?;
            if ordinal < suffix_count {
                selected = Some(candidate);
                break;
            }
            ordinal -= suffix_count;
        }
        let selected = selected.ok_or(BoundarySimplexPlanError::Invariant {
            detail: "lexicographic boundary subset unranking found no axis",
        })?;
        local_indices.push(selected);
        next_candidate = selected + 1;
    }
    if ordinal != 0 {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "lexicographic boundary subset unranking left a residual ordinal",
        });
    }

    let mut pinned = Vec::new();
    try_reserve_exact(&mut pinned, subset_size, "pinned ambient axes")?;
    let mut remaining = Vec::new();
    try_reserve_exact(
        &mut remaining,
        free_axes.len() - subset_size,
        "remaining ambient axes",
    )?;
    let mut pinned_cursor = 0usize;
    for (local_axis, &ambient_axis) in free_axes.iter().enumerate() {
        if local_indices.get(pinned_cursor).copied() == Some(local_axis) {
            pinned.push(ambient_axis);
            pinned_cursor += 1;
        } else {
            remaining.push(ambient_axis);
        }
    }
    if pinned.len() != subset_size || remaining.len() + pinned.len() != free_axes.len() {
        return Err(BoundarySimplexPlanError::Invariant {
            detail: "boundary subset partition lost a parent free axis",
        });
    }
    Ok((pinned, remaining))
}
