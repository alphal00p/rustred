//! Aggregate preflight before exact presentation authentication.

use crate::family::IntegralFamily;

use super::error::FamilyPresentationError;
use super::limits::FamilyPresentationLimits;
use super::model::{CommonMassScale, DenominatorRole, MomentumRouting};

pub(super) fn preflight_presentation_inputs(
    family: &IntegralFamily,
    roles: &[DenominatorRole],
    routing: &MomentumRouting,
    common_scale: Option<&CommonMassScale>,
    limits: FamilyPresentationLimits,
) -> Result<(), FamilyPresentationError> {
    let mut label_bytes = 0usize;
    for label in roles.iter().map(DenominatorRole::id).chain(
        routing
            .source_loop_order()
            .iter()
            .chain(routing.source_external_order())
            .map(String::as_str),
    ) {
        label_bytes = checked_add(
            "presentation role and routing label bytes",
            label_bytes,
            label.len(),
        )?;
    }
    check_limit(
        "presentation role and routing label bytes",
        label_bytes,
        limits.max_role_and_routing_label_bytes,
    )?;

    let mut coefficients = 0usize;
    for row in routing
        .loop_linear()
        .iter()
        .chain(routing.loop_external())
        .chain(routing.external_linear())
    {
        coefficients = checked_add("presentation coefficient inputs", coefficients, row.len())?;
    }
    for physical in roles.iter().filter_map(DenominatorRole::physical) {
        coefficients = checked_add(
            "presentation coefficient inputs",
            coefficients,
            physical.momentum().loop_coefficients().len(),
        )?;
        coefficients = checked_add(
            "presentation coefficient inputs",
            coefficients,
            physical.momentum().external_shift().len(),
        )?;
        coefficients = checked_add("presentation coefficient inputs", coefficients, 1)?;
    }
    if common_scale.is_some() {
        coefficients = checked_add("presentation coefficient inputs", coefficients, 1)?;
    }
    check_limit(
        "presentation coefficient inputs",
        coefficients,
        limits.max_coefficient_inputs,
    )?;

    let mut condition_inputs = coefficients;
    if family.external_count() > 0 {
        condition_inputs = checked_add("presentation condition inputs", condition_inputs, 1)?;
    }
    if common_scale.is_some() {
        condition_inputs = checked_add("presentation condition inputs", condition_inputs, 1)?;
    }
    check_limit(
        "presentation condition inputs",
        condition_inputs,
        limits.max_condition_inputs,
    )
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), FamilyPresentationError> {
    if requested > limit {
        Err(FamilyPresentationError::ResourceLimit {
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
) -> Result<usize, FamilyPresentationError> {
    left.checked_add(right)
        .ok_or(FamilyPresentationError::ResourceCountOverflow { resource })
}
