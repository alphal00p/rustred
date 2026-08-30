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

/// Finite union of arbitrary structural boxes in one sector chart.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct BoxCover {
    arity: usize,
    boxes: Box<[LatticeBox]>,
    limits: CompletionGeometryLimits,
}

impl BoxCover {
    pub(crate) fn try_new(
        arity: usize,
        boxes: impl IntoIterator<Item = LatticeBox>,
        limits: CompletionGeometryLimits,
    ) -> Result<Self, CompletionGeometryError> {
        if arity == 0 {
            return Err(CompletionGeometryError::EmptyCoordinateSpace);
        }
        check_limit("completion coordinate arity", arity, limits.max_arity)?;

        let mut requested = Vec::new();
        let mut requested_coordinate_cells = 0usize;
        for cell in boxes {
            if cell.arity() != arity {
                return Err(CompletionGeometryError::WrongArity {
                    object: "structural cover box",
                    expected: arity,
                    actual: cell.arity(),
                });
            }
            let requested_count =
                checked_add("requested structural cover boxes", requested.len(), 1)?;
            check_limit(
                "requested structural cover boxes",
                requested_count,
                limits.max_requested_boxes,
            )?;
            let endpoint_cells =
                checked_mul("requested structural-cover coordinate cells", arity, 2)?;
            requested_coordinate_cells = checked_add(
                "requested structural-cover coordinate cells",
                requested_coordinate_cells,
                endpoint_cells,
            )?;
            check_limit(
                "requested structural-cover coordinate cells",
                requested_coordinate_cells,
                limits.max_requested_box_coordinate_cells,
            )?;
            requested.try_reserve_exact(1).map_err(|_| {
                CompletionGeometryError::AllocationFailure {
                    resource: "requested structural cover boxes",
                    requested: requested_count,
                }
            })?;
            requested.push(cell);
        }
        requested.sort_unstable();
        requested.dedup();
        Ok(Self {
            arity,
            boxes: requested.into_boxed_slice(),
            limits,
        })
    }

    pub(crate) fn boxes(&self) -> &[LatticeBox] {
        &self.boxes
    }

    pub(crate) fn covers(&self, point: &LatticePoint) -> Result<bool, CompletionGeometryError> {
        if point.arity() != self.arity {
            return Err(CompletionGeometryError::WrongArity {
                object: "structural-cover query point",
                expected: self.arity,
                actual: point.arity(),
            });
        }
        Ok(self.boxes.iter().any(|cell| cell.contains(point)))
    }

    /// Subtract every structural box from `N^r` in stable endpoint order.
    pub(crate) fn uncovered_partition(
        &self,
    ) -> Result<UncoveredPartition, CompletionGeometryError> {
        self.uncovered_within(LatticeBox::try_full(self.arity)?)
    }

    pub(crate) fn uncovered_within(
        &self,
        universe: LatticeBox,
    ) -> Result<UncoveredPartition, CompletionGeometryError> {
        if universe.arity() != self.arity {
            return Err(CompletionGeometryError::WrongArity {
                object: "structural-cover universe",
                expected: self.arity,
                actual: universe.arity(),
            });
        }
        let mut uncovered = Vec::new();
        reserve_box_push(&mut uncovered, self.arity, self.limits)?;
        uncovered.push(universe);
        let mut split_operations = 0usize;

        for cover in &self.boxes {
            let mut next = try_vec("uncovered lattice boxes", uncovered.len())?;
            for cell in uncovered {
                subtract_box(
                    cell,
                    cover,
                    &mut next,
                    &mut split_operations,
                    self.arity,
                    self.limits,
                )?;
            }
            next.sort_unstable();
            uncovered = next;
            if uncovered.is_empty() {
                break;
            }
        }

        Ok(UncoveredPartition::new(uncovered, split_operations))
    }
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
        let mut boxes = Vec::new();
        reserve_box_push(&mut boxes, self.arity, self.limits)?;
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

fn subtract_box(
    cell: LatticeBox,
    cover: &LatticeBox,
    output: &mut Vec<LatticeBox>,
    split_operations: &mut usize,
    arity: usize,
    limits: CompletionGeometryLimits,
) -> Result<(), CompletionGeometryError> {
    if !cell.intersects_box(cover) {
        push_box(output, cell, arity, limits)?;
        return Ok(());
    }
    if cover.contains_box(&cell) {
        return Ok(());
    }

    let mut intersection = cell;
    for position in 0..arity {
        *split_operations = checked_add("structural-box split operations", *split_operations, 1)?;
        check_limit(
            "structural-box split operations",
            *split_operations,
            limits.max_split_operations,
        )?;

        let lower = intersection.lower()[position].max(cover.lower()[position]);
        let upper = min_upper(intersection.upper()[position], cover.upper()[position]);
        if intersection.lower()[position] < lower {
            let outside_upper = lower
                .checked_sub(1)
                .ok_or(CompletionGeometryError::Invariant {
                    detail: "a positive box-intersection lower endpoint could not be decremented",
                })?;
            reserve_box_push(output, arity, limits)?;
            let mut outside = intersection.try_clone_fallible()?;
            outside.set_upper(position, outside_upper);
            output.push(outside);
            intersection.raise_lower(position, lower);
        }
        if upper_strictly_less(upper, intersection.upper()[position]) {
            let upper = upper.ok_or(CompletionGeometryError::Invariant {
                detail: "an infinite box-intersection endpoint was smaller than another endpoint",
            })?;
            let outside_lower =
                upper
                    .checked_add(1)
                    .ok_or(CompletionGeometryError::ResourceCountOverflow {
                        resource: "box-intersection successor coordinate",
                    })?;
            reserve_box_push(output, arity, limits)?;
            let mut outside = intersection.try_clone_fallible()?;
            outside.raise_lower(position, outside_lower);
            output.push(outside);
            intersection.set_upper(position, upper);
        }
    }
    Ok(())
}

fn min_upper(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (None, right) => right,
        (left, None) => left,
        (Some(left), Some(right)) => Some(left.min(right)),
    }
}

fn upper_strictly_less(left: Option<u64>, right: Option<u64>) -> bool {
    match (left, right) {
        (Some(_), None) => true,
        (Some(left), Some(right)) => left < right,
        (None, _) => false,
    }
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
