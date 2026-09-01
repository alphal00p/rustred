use crate::foundry::completion::{LatticePoint, SectorChart};
use crate::identity::IntegralShift;

use super::BoundarySimplexPlanError;
use super::resource::try_reserve_exact;

pub(super) fn try_chart_point_to_target_shift(
    sector: &crate::sector::Mask,
    coordinates: &[u64],
    max_arity: usize,
) -> Result<IntegralShift, BoundarySimplexPlanError> {
    let point = LatticePoint::try_new(coordinates.iter().copied())?;
    let chart = SectorChart::new(sector.clone());
    let target = chart.to_integral(&point)?;
    let mut shift = Vec::new();
    try_reserve_exact(&mut shift, coordinates.len(), "target-shift coordinates")?;
    for (&target_power, corner_power) in target.powers().iter().zip(sector.corner_indices()) {
        shift.push(target_power.checked_sub(corner_power).ok_or(
            BoundarySimplexPlanError::Invariant {
                detail: "a chart point could not be displaced from its sector corner",
            },
        )?);
    }
    IntegralShift::try_new_with_component_limit(shift, max_arity).map_err(Into::into)
}
