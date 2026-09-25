//! Serial exact cell restriction; no matcher authority or thread pool is added.
use super::{engine::Budget, model::*};
use crate::foundry::completion::LatticeBox;

pub(super) struct Cells<'a> {
    source: &'a LatticeBox,
    axis: usize,
    next: Option<u64>,
    upper: u64,
    first: bool,
}

impl<'a> Cells<'a> {
    /// None is a pre-work policy miss. Once Some is returned, any failure is
    /// incomplete work: the caller must never replay the unsplit source.
    pub fn new<const N: usize>(
        source: &'a LatticeBox,
        owner: &[bool; N],
        sign_cell_count: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Self>, OwnerAppliedFailure> {
        let OwnerAppliedCellRefinement::SingleFiniteAxis { max_cardinality } =
            budget.limits.cell_refinement
        else {
            return Ok(None);
        };
        let mut varying = None;
        for (axis, &active) in owner.iter().enumerate() {
            let lower = source.lower()[axis];
            let Some(upper) = source.upper()[axis] else {
                return Ok(None);
            };
            // Fixed restriction uses signed physical powers, not local offsets.
            // Refuse this optional plan before any child if either endpoint is
            // outside that existing API; do not create a new numerical failure.
            for endpoint in [lower, upper] {
                let value = if active {
                    i128::from(endpoint) + 1
                } else {
                    -i128::from(endpoint)
                };
                if i64::try_from(value).is_err() {
                    return Ok(None);
                }
            }
            if lower != upper {
                if varying.is_some() {
                    return Ok(None);
                }
                let cardinality = u128::from(upper)
                    .checked_sub(u128::from(lower))
                    .and_then(|width| width.checked_add(1))
                    .ok_or(OwnerAppliedFailure::InternalInvariant(
                        "invalid refinement endpoints",
                    ))?;
                if cardinality > max_cardinality.get() as u128 {
                    return Ok(None);
                }
                let cardinality = usize::try_from(cardinality).map_err(|_| {
                    OwnerAppliedFailure::CountOverflow {
                        resource: "application cell cardinality",
                    }
                })?;
                varying = Some((axis, lower, upper, cardinality));
            }
        }
        let Some((axis, lower, upper, cardinality)) = varying else {
            return Ok(None);
        };
        budget.cancelled()?;
        // Boundaries::next already charged the original cell. Reuse that one
        // charge and prospectively admit every extra singleton before emitting
        // any. Actual incremental charges are committed as children are visited.
        let requested = budget
            .stats
            .boundary_cells
            .checked_add(cardinality - 1)
            .ok_or(OwnerAppliedFailure::CountOverflow {
                resource: "boundary cells",
            })?;
        budget.check(
            requested,
            budget.limits.max_boundary_cells,
            "boundary cells",
        )?;
        // The original cell remains borrowed while one child is alive. Keep
        // the original sign-partition high-water allowance (count + 12), plus
        // that extra child, even if some sign cells have already been consumed.
        // No collection proportional to refinement cardinality is allocated.
        let scratch_boxes =
            sign_cell_count
                .checked_add(13)
                .ok_or(OwnerAppliedFailure::CountOverflow {
                    resource: "scratch boxes",
                })?;
        budget.check(
            scratch_boxes,
            budget.limits.max_scratch_boxes,
            "scratch boxes",
        )?;
        let coordinates = N
            .checked_mul(2)
            .and_then(|n| n.checked_mul(scratch_boxes))
            .ok_or(OwnerAppliedFailure::CountOverflow {
                resource: "scratch coordinate cells",
            })?;
        budget.check(
            coordinates,
            budget.limits.max_scratch_coordinate_cells,
            "scratch coordinate cells",
        )?;
        budget.refinement_step()?;
        Ok(Some(Self {
            source,
            axis,
            next: Some(lower),
            upper,
            first: true,
        }))
    }

    pub fn next(
        &mut self,
        budget: &mut Budget<'_>,
    ) -> Result<Option<LatticeBox>, OwnerAppliedFailure> {
        let Some(value) = self.next else {
            return Ok(None);
        };
        budget.cancelled()?;
        if !self.first {
            budget.boundary()?;
        }
        budget.refinement_cell()?;
        self.first = false;
        self.next = if value == self.upper {
            None
        } else {
            Some(
                value
                    .checked_add(1)
                    .ok_or(OwnerAppliedFailure::CountOverflow {
                        resource: "application cell endpoint",
                    })?,
            )
        };
        let mut lower = self.source.lower().to_vec();
        let mut upper = self.source.upper().to_vec();
        lower[self.axis] = value;
        upper[self.axis] = Some(value);
        LatticeBox::try_new(lower, upper)
            .map(Some)
            .map_err(|e| OwnerAppliedFailure::Geometry(e.to_string()))
    }
}

#[cfg(test)]
mod tests;
