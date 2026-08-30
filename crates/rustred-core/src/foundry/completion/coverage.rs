use super::model::try_vec;
use super::{
    CompletionGeometryError, CompletionGeometryLimits, LatticeBox, LatticePoint, UncoveredPartition,
};

/// Minimal antichain of leading coordinates and its exact upward closure.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LeadingIdeal {
    arity: usize,
    generators: Box<[LatticePoint]>,
    limits: CompletionGeometryLimits,
}

impl LeadingIdeal {
    pub(crate) fn try_new(
        arity: usize,
        generators: impl IntoIterator<Item = LatticePoint>,
        limits: CompletionGeometryLimits,
    ) -> Result<Self, CompletionGeometryError> {
        if arity == 0 {
            return Err(CompletionGeometryError::EmptyCoordinateSpace);
        }
        check_limit("completion coordinate arity", arity, limits.max_arity)?;

        let mut requested = Vec::new();
        let mut requested_coordinate_cells = 0usize;
        for generator in generators {
            let ordinal = requested.len();
            if generator.arity() != arity {
                return Err(CompletionGeometryError::WrongArity {
                    object: "leading-ideal generator",
                    expected: arity,
                    actual: generator.arity(),
                });
            }
            let requested_count = checked_add("requested leading-ideal generators", ordinal, 1)?;
            check_limit(
                "requested leading-ideal generators",
                requested_count,
                limits.max_requested_generators,
            )?;
            requested_coordinate_cells = checked_add(
                "requested leading-generator coordinate cells",
                requested_coordinate_cells,
                arity,
            )?;
            check_limit(
                "requested leading-generator coordinate cells",
                requested_coordinate_cells,
                limits.max_requested_generator_coordinate_cells,
            )?;
            requested.try_reserve_exact(1).map_err(|_| {
                CompletionGeometryError::AllocationFailure {
                    resource: "requested leading-ideal generators",
                    requested: requested_count,
                }
            })?;
            requested.push(generator);
        }
        requested.sort_unstable();
        requested.dedup();

        let mut minimal = try_vec("minimal leading-ideal generators", requested.len())?;
        for generator in requested {
            if minimal
                .iter()
                .any(|existing| componentwise_le(existing, &generator))
            {
                continue;
            }
            minimal.retain(|existing| !componentwise_le(&generator, existing));
            let retained = checked_add("minimal leading-ideal generators", minimal.len(), 1)?;
            check_limit(
                "minimal leading-ideal generators",
                retained,
                limits.max_minimal_generators,
            )?;
            minimal.push(generator);
        }
        minimal.sort_unstable();
        Ok(Self {
            arity,
            generators: minimal.into_boxed_slice(),
            limits,
        })
    }

    pub(crate) fn arity(&self) -> usize {
        self.arity
    }

    pub(crate) fn generators(&self) -> &[LatticePoint] {
        &self.generators
    }

    pub(crate) fn covers(&self, point: &LatticePoint) -> Result<bool, CompletionGeometryError> {
        if point.arity() != self.arity {
            return Err(CompletionGeometryError::WrongArity {
                object: "leading-ideal query point",
                expected: self.arity,
                actual: point.arity(),
            });
        }
        Ok(self
            .generators
            .iter()
            .any(|generator| componentwise_le(generator, point)))
    }

    /// Subtract every leading orthant from `N^r` in stable antichain order.
    ///
    /// Each subtraction emits disjoint first-violating-coordinate pieces, so
    /// the returned boxes are mutually disjoint and cover the complement
    /// exactly.  No finite probe depth participates in the construction.
    pub(crate) fn uncovered_partition(
        &self,
    ) -> Result<UncoveredPartition, CompletionGeometryError> {
        let mut boxes = try_vec("uncovered lattice boxes", 1)?;
        boxes.push(LatticeBox::try_full(self.arity)?);
        let mut split_operations = 0usize;

        for generator in &self.generators {
            let mut next = try_vec("uncovered lattice boxes", boxes.len())?;
            for cell in boxes {
                subtract_orthant(
                    cell,
                    generator,
                    &mut next,
                    &mut split_operations,
                    self.arity,
                    self.limits,
                )?;
            }
            next.sort_unstable();
            boxes = next;
            if boxes.is_empty() {
                break;
            }
        }

        Ok(UncoveredPartition::new(boxes, split_operations))
    }
}

fn subtract_orthant(
    cell: LatticeBox,
    origin: &LatticePoint,
    output: &mut Vec<LatticeBox>,
    split_operations: &mut usize,
    arity: usize,
    limits: CompletionGeometryLimits,
) -> Result<(), CompletionGeometryError> {
    if !cell.intersects_orthant(origin) {
        push_box(output, cell, arity, limits)?;
        return Ok(());
    }
    if cell.is_inside_orthant(origin) {
        return Ok(());
    }

    let mut intersection = cell;
    for (position, &threshold) in origin.coordinates().iter().enumerate() {
        *split_operations = checked_add("leading-orthant split operations", *split_operations, 1)?;
        check_limit(
            "leading-orthant split operations",
            *split_operations,
            limits.max_split_operations,
        )?;
        if intersection.lower()[position] < threshold {
            let upper = threshold
                .checked_sub(1)
                .ok_or(CompletionGeometryError::Invariant {
                    detail: "a positive orthant threshold could not be decremented",
                })?;
            reserve_box_push(output, arity, limits)?;
            let mut outside = intersection.try_clone_fallible()?;
            outside.set_upper(position, upper);
            output.push(outside);
        }
        intersection.raise_lower(position, threshold);
    }
    Ok(())
}

fn push_box(
    output: &mut Vec<LatticeBox>,
    cell: LatticeBox,
    arity: usize,
    limits: CompletionGeometryLimits,
) -> Result<(), CompletionGeometryError> {
    reserve_box_push(output, arity, limits)?;
    output.push(cell);
    Ok(())
}

fn reserve_box_push(
    output: &mut Vec<LatticeBox>,
    arity: usize,
    limits: CompletionGeometryLimits,
) -> Result<(), CompletionGeometryError> {
    let requested = checked_add("uncovered lattice boxes", output.len(), 1)?;
    preflight_box_push(output.len(), arity, limits)?;
    output
        .try_reserve_exact(1)
        .map_err(|_| CompletionGeometryError::AllocationFailure {
            resource: "uncovered lattice boxes",
            requested,
        })?;
    Ok(())
}

fn preflight_box_push(
    retained: usize,
    arity: usize,
    limits: CompletionGeometryLimits,
) -> Result<(), CompletionGeometryError> {
    let requested = checked_add("uncovered lattice boxes", retained, 1)?;
    check_limit(
        "uncovered lattice boxes",
        requested,
        limits.max_uncovered_boxes,
    )?;
    let endpoints = checked_mul("uncovered-box coordinate cells", requested, arity)?;
    let coordinate_cells = checked_mul("uncovered-box coordinate cells", endpoints, 2)?;
    check_limit(
        "uncovered-box coordinate cells",
        coordinate_cells,
        limits.max_uncovered_box_coordinate_cells,
    )
}

fn componentwise_le(left: &LatticePoint, right: &LatticePoint) -> bool {
    left.arity() == right.arity()
        && left
            .coordinates()
            .iter()
            .zip(right.coordinates())
            .all(|(&left, &right)| left <= right)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), CompletionGeometryError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(CompletionGeometryError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CompletionGeometryError> {
    left.checked_add(right)
        .ok_or(CompletionGeometryError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, CompletionGeometryError> {
    left.checked_mul(right)
        .ok_or(CompletionGeometryError::ResourceCountOverflow { resource })
}
