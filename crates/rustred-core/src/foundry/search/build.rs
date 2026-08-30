use crate::family::IntegralKey;
use crate::identity::IntegralShift;

use super::{SectorSearchDiamond, SectorSearchError, SectorSearchLimits};

const RETAINED_OFFSETS: &str = "sector-search retained offsets";
const OFFSET_COORDINATE_CELLS: &str = "sector-search retained offset coordinate cells";

impl SectorSearchDiamond {
    /// Build the complete L1 ball of `depth` around `anchor`, restricted to
    /// offsets whose shifted point remains in the anchor's exact sector.
    ///
    /// Exact output cardinality and coordinate storage are checked before any
    /// output offset is allocated. Enumeration is iterative and emits the
    /// retained offsets directly in lexicographic order.
    pub fn try_new(
        anchor: IntegralKey,
        depth: usize,
        limits: SectorSearchLimits,
    ) -> Result<Self, SectorSearchError> {
        check_limit("sector-search depth", depth, limits.max_depth)?;
        let lattice_depth =
            i64::try_from(depth).map_err(|_| SectorSearchError::DepthNotRepresentable { depth })?;

        let retained_count = count_retained_offsets(&anchor, depth, lattice_depth)?;
        check_limit(RETAINED_OFFSETS, retained_count, limits.max_offsets)?;
        let coordinate_cells = checked_mul(
            OFFSET_COORDINATE_CELLS,
            retained_count,
            anchor.powers().len(),
        )?;
        check_limit(
            OFFSET_COORDINATE_CELLS,
            coordinate_cells,
            limits.max_offset_coordinate_cells,
        )?;

        let offsets = enumerate_offsets(&anchor, depth, lattice_depth, retained_count)?;
        Ok(Self {
            anchor,
            depth,
            offsets: offsets.into_boxed_slice(),
        })
    }
}

fn count_retained_offsets(
    anchor: &IntegralKey,
    depth: usize,
    lattice_depth: i64,
) -> Result<usize, SectorSearchError> {
    let count_cells = checked_add("sector-search count cells", depth, 1)?;
    let mut counts = try_zeroed_vec(count_cells, "sector-search count cells")?;
    let mut next = try_zeroed_vec(count_cells, "sector-search count cells")?;
    counts[0] = 1;

    for &power in anchor.powers() {
        next.fill(0);
        let (minimum, maximum) = same_sector_offset_bounds(power, lattice_depth);
        for used in 0..=depth {
            let prefix_count = counts[used];
            if prefix_count == 0 {
                continue;
            }
            for magnitude in 0..=depth - used {
                let multiplicity = magnitude_multiplicity(minimum, maximum, magnitude)?;
                if multiplicity == 0 {
                    continue;
                }
                let contribution = checked_mul(RETAINED_OFFSETS, prefix_count, multiplicity)?;
                next[used + magnitude] =
                    checked_add(RETAINED_OFFSETS, next[used + magnitude], contribution)?;
            }
        }
        std::mem::swap(&mut counts, &mut next);
    }

    counts.into_iter().try_fold(0usize, |total, count| {
        checked_add(RETAINED_OFFSETS, total, count)
    })
}

fn enumerate_offsets(
    anchor: &IntegralKey,
    depth: usize,
    lattice_depth: i64,
    retained_count: usize,
) -> Result<Vec<IntegralShift>, SectorSearchError> {
    let arity = anchor.powers().len();
    let mut offsets = try_vec(retained_count, RETAINED_OFFSETS)?;
    let mut values = try_zeroed_i64_vec(arity, "sector-search enumeration coordinates")?;
    let mut next_values = try_none_vec(arity, "sector-search enumeration cursors")?;
    let remaining_cells = checked_add("sector-search enumeration budgets", arity, 1)?;
    let mut remaining = try_zeroed_vec(remaining_cells, "sector-search enumeration budgets")?;
    remaining[0] = depth;

    let (minimum, _) = bounded_coordinate_range(anchor.powers()[0], lattice_depth, depth)?;
    next_values[0] = Some(minimum);
    let mut position = 0usize;

    loop {
        if position == arity {
            offsets.push(
                IntegralShift::try_new_with_component_limit(values.iter().copied(), arity)
                    .map_err(SectorSearchError::IntegralShift)?,
            );
            position -= 1;
            continue;
        }

        let Some(value) = next_values[position] else {
            if position == 0 {
                break;
            }
            position -= 1;
            continue;
        };
        let (_, maximum) = bounded_coordinate_range(
            anchor.powers()[position],
            lattice_depth,
            remaining[position],
        )?;
        next_values[position] = if value == maximum {
            None
        } else {
            Some(value + 1)
        };
        values[position] = value;
        let magnitude =
            usize::try_from(value.unsigned_abs()).map_err(|_| SectorSearchError::Invariant {
                detail: "one admitted offset magnitude does not fit usize",
            })?;
        remaining[position + 1] = remaining[position] - magnitude;
        position += 1;
        if position < arity {
            let (minimum, _) = bounded_coordinate_range(
                anchor.powers()[position],
                lattice_depth,
                remaining[position],
            )?;
            next_values[position] = Some(minimum);
        }
    }

    if offsets.len() != retained_count {
        return Err(SectorSearchError::Invariant {
            detail: "enumeration did not reproduce the preflighted offset count",
        });
    }
    Ok(offsets)
}

fn bounded_coordinate_range(
    power: i64,
    lattice_depth: i64,
    remaining: usize,
) -> Result<(i64, i64), SectorSearchError> {
    let remaining = i64::try_from(remaining).map_err(|_| SectorSearchError::Invariant {
        detail: "one admitted coordinate budget is not representable by i64",
    })?;
    let (sector_minimum, sector_maximum) = same_sector_offset_bounds(power, lattice_depth);
    Ok((
        sector_minimum.max(-remaining),
        sector_maximum.min(remaining),
    ))
}

fn same_sector_offset_bounds(power: i64, depth: i64) -> (i64, i64) {
    let power = i128::from(power);
    let depth = i128::from(depth);
    let (minimum_power, maximum_power) = if power >= 1 {
        (i128::from(1), i128::from(i64::MAX))
    } else {
        (i128::from(i64::MIN), i128::from(0))
    };
    let minimum = (-depth).max(minimum_power - power);
    let maximum = depth.min(maximum_power - power);
    (
        i64::try_from(minimum).expect("the lower offset bound is intersected with i64 depth"),
        i64::try_from(maximum).expect("the upper offset bound is intersected with i64 depth"),
    )
}

fn magnitude_multiplicity(
    minimum: i64,
    maximum: i64,
    magnitude: usize,
) -> Result<usize, SectorSearchError> {
    if magnitude == 0 {
        return Ok(usize::from(minimum <= 0 && maximum >= 0));
    }
    let magnitude = i64::try_from(magnitude).map_err(|_| SectorSearchError::Invariant {
        detail: "one admitted counting magnitude is not representable by i64",
    })?;
    Ok(usize::from(-magnitude >= minimum) + usize::from(magnitude <= maximum))
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SectorSearchError> {
    if requested > limit {
        Err(SectorSearchError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SectorSearchError> {
    left.checked_add(right)
        .ok_or(SectorSearchError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SectorSearchError> {
    left.checked_mul(right)
        .ok_or(SectorSearchError::ResourceCountOverflow { resource })
}

fn try_vec<T>(capacity: usize, resource: &'static str) -> Result<Vec<T>, SectorSearchError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| SectorSearchError::AllocationFailure {
            resource,
            requested: capacity,
        })?;
    Ok(values)
}

fn try_zeroed_vec(len: usize, resource: &'static str) -> Result<Vec<usize>, SectorSearchError> {
    let mut values = try_vec(len, resource)?;
    values.resize(len, 0);
    Ok(values)
}

fn try_zeroed_i64_vec(len: usize, resource: &'static str) -> Result<Vec<i64>, SectorSearchError> {
    let mut values = try_vec(len, resource)?;
    values.resize(len, 0);
    Ok(values)
}

fn try_none_vec(len: usize, resource: &'static str) -> Result<Vec<Option<i64>>, SectorSearchError> {
    let mut values = try_vec(len, resource)?;
    values.resize(len, None);
    Ok(values)
}

#[cfg(test)]
pub(super) fn checked_coordinate_cells_for_test(
    offsets: usize,
    arity: usize,
) -> Result<usize, SectorSearchError> {
    checked_mul(OFFSET_COORDINATE_CELLS, offsets, arity)
}
