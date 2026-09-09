use crate::identity::IntegralShift;
use crate::sector::{InteriorBounds, SectorInteriorDomain, SectorMonotoneDomain};

use super::{
    SpiredConditionallyAllowedInactiveActivation, SpiredInactiveActivationAxis,
    SpiredInactiveActivationError, SpiredInactiveActivationLimits, SpiredInactiveActivationSlice,
};

/// Split the exact parent box into a safe interior and finite activation cells.
///
/// This is structural geometry only. In particular, returning `Some` does not
/// admit the shifted column: the combined exact circuit coefficient must later
/// vanish on every activation cell on which a rule is claimed.
pub(crate) fn try_decompose_inactive_activation(
    parent_domain: &SectorMonotoneDomain,
    shift: &IntegralShift,
    limits: SpiredInactiveActivationLimits,
) -> Result<Option<SpiredConditionallyAllowedInactiveActivation>, SpiredInactiveActivationError> {
    let arity = parent_domain.arity();
    if shift.len() != arity {
        return Err(SpiredInactiveActivationError::WrongShiftArity {
            expected: arity,
            actual: shift.len(),
        });
    }
    check_limit("index-space arity", arity, limits.max_arity)?;

    let mut affected_axes = Vec::new();
    affected_axes
        .try_reserve(arity.min(limits.max_affected_axes))
        .map_err(|_| SpiredInactiveActivationError::AllocationFailure {
            resource: "affected axes",
            requested: arity.min(limits.max_affected_axes),
        })?;

    for (position, ((&active, &bounds), &component)) in parent_domain
        .sector()
        .active_bits()
        .iter()
        .zip(parent_domain.bounds())
        .zip(shift.values())
        .enumerate()
    {
        if active || component <= 0 {
            continue;
        }

        // An inactive line becomes active exactly when n_i+s_i >= 1.  Since
        // n_i<=0 on the parent sector, this is a finite interval of at most
        // s_i integer values, even when the parent interval is unbounded below.
        let activation_lower_i128 = i128::from(bounds.lower()).max(1_i128 - i128::from(component));
        let activation_upper_i128 = i128::from(bounds.upper());
        if activation_lower_i128 > activation_upper_i128 {
            continue;
        }
        let activation_lower = i64::try_from(activation_lower_i128).map_err(|_| {
            SpiredInactiveActivationError::Invariant {
                detail: "inactive activation lower endpoint escaped i64",
            }
        })?;
        let activation_upper = bounds.upper();
        let activation_value_count = usize::try_from(
            activation_upper_i128 - activation_lower_i128 + 1_i128,
        )
        .map_err(|_| SpiredInactiveActivationError::ResourceCountOverflow {
            resource: "activation slices",
        })?;

        let safe_upper_i128 = i128::from(bounds.upper()).min(-i128::from(component));
        let safe_bounds = (i128::from(bounds.lower()) <= safe_upper_i128)
            .then(|| {
                i64::try_from(safe_upper_i128)
                    .map(|safe_upper| InteriorBounds::new(bounds.lower(), safe_upper))
            })
            .transpose()
            .map_err(|_| SpiredInactiveActivationError::Invariant {
                detail: "inactive safe-interior endpoint escaped i64",
            })?;

        let requested = checked_add("affected axes", affected_axes.len(), 1)?;
        check_limit("affected axes", requested, limits.max_affected_axes)?;
        affected_axes.push(SpiredInactiveActivationAxis::new(
            position,
            component,
            safe_bounds,
            InteriorBounds::new(activation_lower, activation_upper),
            activation_value_count,
        ));
    }

    if affected_axes.is_empty() {
        return Ok(None);
    }

    // Preflight the reachable first-activation partition before allocating any
    // domain buffers.  If an earlier axis has no safe part, later axes can
    // never be the first activating coordinate and therefore retain no cells.
    let mut slice_count = 0usize;
    let mut safe_interior_exists = true;
    for axis in &affected_axes {
        if !safe_interior_exists {
            break;
        }
        slice_count = checked_add(
            "activation slices",
            slice_count,
            axis.activation_value_count(),
        )?;
        check_limit(
            "activation slices",
            slice_count,
            limits.max_activation_slices,
        )?;
        safe_interior_exists = axis.safe_bounds().is_some();
    }

    let retained_domains = checked_add(
        "retained domains",
        slice_count,
        usize::from(safe_interior_exists),
    )?;
    check_limit(
        "retained domains",
        retained_domains,
        limits.max_retained_domains,
    )?;
    let retained_domain_bound_cells =
        checked_mul("retained domain bound cells", retained_domains, arity)?;
    check_limit(
        "retained domain bound cells",
        retained_domain_bound_cells,
        limits.max_retained_domain_bound_cells,
    )?;

    let mut activation_slices = Vec::new();
    activation_slices
        .try_reserve_exact(slice_count)
        .map_err(|_| SpiredInactiveActivationError::AllocationFailure {
            resource: "activation slices",
            requested: slice_count,
        })?;
    let mut prefix_bounds = try_copy_bounds(parent_domain.bounds())?;
    for (axis_ordinal, axis) in affected_axes.iter().enumerate() {
        for activation_value in axis.activation_bounds().lower()..=axis.activation_bounds().upper()
        {
            let mut slice_bounds = try_copy_bounds(&prefix_bounds)?;
            slice_bounds[axis.position()] = InteriorBounds::new(activation_value, activation_value);
            let domain =
                SectorInteriorDomain::try_new(parent_domain.sector().clone(), slice_bounds)?;
            activation_slices.push(SpiredInactiveActivationSlice::new(
                axis_ordinal,
                axis.position(),
                activation_value,
                domain,
            ));
        }
        let Some(safe_bounds) = axis.safe_bounds() else {
            prefix_bounds.clear();
            break;
        };
        prefix_bounds[axis.position()] = safe_bounds;
    }

    if activation_slices.len() != slice_count {
        return Err(SpiredInactiveActivationError::Invariant {
            detail: "materialized activation-slice count disagrees with preflight",
        });
    }
    let safe_interior = if safe_interior_exists {
        Some(SectorInteriorDomain::try_new(
            parent_domain.sector().clone(),
            prefix_bounds,
        )?)
    } else {
        None
    };

    Ok(Some(
        SpiredConditionallyAllowedInactiveActivation::from_parts(
            parent_domain.clone(),
            shift.clone(),
            affected_axes,
            safe_interior,
            activation_slices,
            retained_domain_bound_cells,
        ),
    ))
}

fn try_copy_bounds(
    bounds: &[InteriorBounds],
) -> Result<Vec<InteriorBounds>, SpiredInactiveActivationError> {
    let mut copied = Vec::new();
    copied.try_reserve_exact(bounds.len()).map_err(|_| {
        SpiredInactiveActivationError::AllocationFailure {
            resource: "retained domain bounds",
            requested: bounds.len(),
        }
    })?;
    copied.extend_from_slice(bounds);
    Ok(copied)
}

fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredInactiveActivationError> {
    left.checked_add(right)
        .ok_or(SpiredInactiveActivationError::ResourceCountOverflow { resource })
}

fn checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, SpiredInactiveActivationError> {
    left.checked_mul(right)
        .ok_or(SpiredInactiveActivationError::ResourceCountOverflow { resource })
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), SpiredInactiveActivationError> {
    if requested <= limit {
        Ok(())
    } else {
        Err(SpiredInactiveActivationError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    }
}
