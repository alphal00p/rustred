//! Presentation-specific generic-domain conditions.

use super::error::{FamilyPresentationError, PresentationCoefficientLocation};
use super::limits::FamilyPresentationLimits;
use super::model::{
    CommonMassScale, DenominatorRole, MomentumRouting, PresentationConditionSource,
    PresentationDomain, PresentationNonZeroCondition,
};
use crate::algebra::{Coefficient, CoefficientPolynomial};

pub(super) fn build_presentation_domain(
    roles: &[DenominatorRole],
    routing: &MomentumRouting,
    common_scale: Option<&CommonMassScale>,
    external_routing_determinant: Option<&Coefficient>,
    limits: FamilyPresentationLimits,
) -> Result<PresentationDomain, FamilyPresentationError> {
    let mut conditions = Vec::new();
    let mut admission = ConditionAdmission::new(limits);

    for (row, values) in routing.loop_linear().iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            add_coefficient_denominator(
                &mut conditions,
                &mut admission,
                coefficient,
                PresentationCoefficientLocation::RoutingLoopLinear { row, column },
            )?;
        }
    }
    for (row, values) in routing.loop_external().iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            add_coefficient_denominator(
                &mut conditions,
                &mut admission,
                coefficient,
                PresentationCoefficientLocation::RoutingLoopExternal { row, column },
            )?;
        }
    }
    for (row, values) in routing.external_linear().iter().enumerate() {
        for (column, coefficient) in values.iter().enumerate() {
            add_coefficient_denominator(
                &mut conditions,
                &mut admission,
                coefficient,
                PresentationCoefficientLocation::RoutingExternalLinear { row, column },
            )?;
        }
    }
    if let Some(determinant) = external_routing_determinant {
        add_condition(
            &mut conditions,
            &mut admission,
            &determinant.numerator,
            PresentationConditionSource::ExternalRoutingDeterminantNumerator,
        )?;
    }
    for (denominator, role) in roles.iter().enumerate() {
        let Some(physical) = role.physical() else {
            continue;
        };
        for (loop_index, coefficient) in physical.momentum().loop_coefficients().iter().enumerate()
        {
            add_coefficient_denominator(
                &mut conditions,
                &mut admission,
                coefficient,
                PresentationCoefficientLocation::PhysicalLoopCoefficient {
                    denominator,
                    loop_index,
                },
            )?;
        }
        for (external, coefficient) in physical.momentum().external_shift().iter().enumerate() {
            add_coefficient_denominator(
                &mut conditions,
                &mut admission,
                coefficient,
                PresentationCoefficientLocation::PhysicalExternalShift {
                    denominator,
                    external,
                },
            )?;
        }
        add_coefficient_denominator(
            &mut conditions,
            &mut admission,
            physical.mass_squared(),
            PresentationCoefficientLocation::PhysicalMassSquared { denominator },
        )?;
    }
    if let Some(common_scale) = common_scale {
        add_coefficient_denominator(
            &mut conditions,
            &mut admission,
            common_scale.scale_squared(),
            PresentationCoefficientLocation::CommonMassScaleSquared,
        )?;
        // Non-identical zero in K(theta) is a generic-domain statement.  Keep
        // the exact numerator guard that makes the common scale nonzero.
        add_condition(
            &mut conditions,
            &mut admission,
            &common_scale.scale_squared().numerator,
            PresentationConditionSource::CommonMassScaleNumerator,
        )?;
    }
    Ok(PresentationDomain { conditions })
}

fn add_coefficient_denominator(
    conditions: &mut Vec<PresentationNonZeroCondition>,
    admission: &mut ConditionAdmission,
    coefficient: &Coefficient,
    location: PresentationCoefficientLocation,
) -> Result<(), FamilyPresentationError> {
    if coefficient.denominator.is_one() {
        return Ok(());
    }
    add_condition(
        conditions,
        admission,
        &coefficient.denominator,
        PresentationConditionSource::CoefficientDenominator(location),
    )
}

fn add_condition(
    conditions: &mut Vec<PresentationNonZeroCondition>,
    admission: &mut ConditionAdmission,
    polynomial: &CoefficientPolynomial,
    source: PresentationConditionSource,
) -> Result<(), FamilyPresentationError> {
    if polynomial.is_zero() {
        return Err(FamilyPresentationError::ZeroNonZeroCondition { source });
    }
    if polynomial.is_constant() {
        return Ok(());
    }
    if let Some(existing) = conditions
        .iter_mut()
        .find(|condition| condition.polynomial == *polynomial)
    {
        if existing.sources.contains(&source) {
            return Ok(());
        }
        let next_sources = admission.next_source_count()?;
        existing.sources.try_reserve_exact(1).map_err(|_| {
            FamilyPresentationError::AllocationFailure {
                resource: "presentation condition sources",
                requested: existing.sources.len().saturating_add(1),
            }
        })?;
        existing.sources.push(source);
        existing.sources.sort_unstable();
        admission.condition_sources = next_sources;
        return Ok(());
    }

    let next_conditions = admission.next_condition_count()?;
    let next_sources = admission.next_source_count()?;
    conditions
        .try_reserve_exact(1)
        .map_err(|_| FamilyPresentationError::AllocationFailure {
            resource: "presentation nonzero conditions",
            requested: next_conditions,
        })?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(1)
        .map_err(|_| FamilyPresentationError::AllocationFailure {
            resource: "presentation condition sources",
            requested: 1,
        })?;
    sources.push(source);
    // Symbolica's public polynomial representation has no fallible deep-clone
    // operation.  The authenticated exact-algebra limits have already
    // censused this payload before this sole retained condition clone.
    conditions.push(PresentationNonZeroCondition {
        polynomial: polynomial.clone(),
        sources,
    });
    admission.conditions = next_conditions;
    admission.condition_sources = next_sources;
    Ok(())
}

struct ConditionAdmission {
    limits: FamilyPresentationLimits,
    conditions: usize,
    condition_sources: usize,
}

impl ConditionAdmission {
    const fn new(limits: FamilyPresentationLimits) -> Self {
        Self {
            limits,
            conditions: 0,
            condition_sources: 0,
        }
    }

    fn next_condition_count(&self) -> Result<usize, FamilyPresentationError> {
        let requested = self.conditions.checked_add(1).ok_or(
            FamilyPresentationError::ResourceCountOverflow {
                resource: "presentation nonzero conditions",
            },
        )?;
        super::admission::check_limit(
            "presentation nonzero conditions",
            requested,
            self.limits.max_nonzero_conditions,
        )?;
        Ok(requested)
    }

    fn next_source_count(&self) -> Result<usize, FamilyPresentationError> {
        let requested = self.condition_sources.checked_add(1).ok_or(
            FamilyPresentationError::ResourceCountOverflow {
                resource: "presentation condition sources",
            },
        )?;
        super::admission::check_limit(
            "presentation condition sources",
            requested,
            self.limits.max_condition_sources,
        )?;
        Ok(requested)
    }
}
