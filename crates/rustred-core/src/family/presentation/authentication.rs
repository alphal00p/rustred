//! Exact authentication of presentation metadata.

use crate::algebra::Coefficient;
use crate::algebra::matrix::{SymbolicaCoefficientMatrixError, determinant_of_coefficient_matrix};
use crate::family::IntegralFamily;

use super::error::{FamilyPresentationError, PresentationCoefficientLocation};
use super::limits::FamilyPresentationLimits;
use super::model::{
    CommonMassScale, DenominatorRole, FamilyConventions, FamilyPresentation, MomentumRouting,
};
use super::replay::verify_physical_denominator;

impl FamilyPresentation {
    /// Consume an exact family, replay its physical/scale claims, and admit
    /// structurally validated caller-attested routing/convention metadata.
    pub fn try_new(
        family: IntegralFamily,
        denominator_roles: Vec<DenominatorRole>,
        routing: MomentumRouting,
        conventions: FamilyConventions,
        common_mass_scale: Option<CommonMassScale>,
    ) -> Result<Self, FamilyPresentationError> {
        Self::try_new_with_limits(
            family,
            denominator_roles,
            routing,
            conventions,
            common_mass_scale,
            FamilyPresentationLimits::default(),
        )
    }

    /// Admit a presentation under explicit aggregate limits and the proof
    /// boundary documented by [`FamilyPresentation`].
    pub fn try_new_with_limits(
        family: IntegralFamily,
        denominator_roles: Vec<DenominatorRole>,
        routing: MomentumRouting,
        conventions: FamilyConventions,
        common_mass_scale: Option<CommonMassScale>,
        limits: FamilyPresentationLimits,
    ) -> Result<Self, FamilyPresentationError> {
        if denominator_roles.len() != family.denominator_count() {
            return Err(FamilyPresentationError::WrongDenominatorRoleCount {
                expected: family.denominator_count(),
                actual: denominator_roles.len(),
            });
        }
        super::admission::preflight_presentation_inputs(
            &family,
            &denominator_roles,
            &routing,
            common_mass_scale.as_ref(),
            limits,
        )?;
        validate_roles(&family, &denominator_roles)?;
        let external_routing_determinant = validate_routing(&family, &routing)?;
        for (denominator, role) in denominator_roles.iter().enumerate() {
            if let DenominatorRole::Physical(physical) = role {
                verify_physical_denominator(&family, denominator, physical, conventions)?;
            }
        }
        validate_common_scale(&family, &denominator_roles, common_mass_scale.as_ref())?;
        let domain = super::domain::build_presentation_domain(
            &denominator_roles,
            &routing,
            common_mass_scale.as_ref(),
            external_routing_determinant.as_ref(),
            limits,
        )?;
        Ok(Self {
            family,
            denominator_roles,
            routing,
            conventions,
            common_mass_scale,
            domain,
            limits,
        })
    }
}

fn validate_roles(
    family: &IntegralFamily,
    roles: &[DenominatorRole],
) -> Result<(), FamilyPresentationError> {
    if roles.len() != family.denominator_count() {
        return Err(FamilyPresentationError::WrongDenominatorRoleCount {
            expected: family.denominator_count(),
            actual: roles.len(),
        });
    }
    for (denominator, role) in roles.iter().enumerate() {
        if role.id().is_empty() {
            return Err(FamilyPresentationError::EmptyDenominatorId { denominator });
        }
        if roles[..denominator]
            .iter()
            .any(|candidate| candidate.id() == role.id())
        {
            return Err(FamilyPresentationError::DuplicateDenominatorId {
                denominator,
                id: try_copy_string(role.id(), "duplicate presentation denominator ID")?,
            });
        }
        let Some(physical) = role.physical() else {
            continue;
        };
        validate_physical_arity(
            denominator,
            "loop-momentum",
            physical.momentum().loop_coefficients().len(),
            family.loop_count(),
        )?;
        validate_physical_arity(
            denominator,
            "external-shift",
            physical.momentum().external_shift().len(),
            family.external_count(),
        )?;
        for (loop_index, coefficient) in physical.momentum().loop_coefficients().iter().enumerate()
        {
            validate_coefficient(
                family,
                coefficient,
                PresentationCoefficientLocation::PhysicalLoopCoefficient {
                    denominator,
                    loop_index,
                },
            )?;
        }
        if physical
            .momentum()
            .loop_coefficients()
            .iter()
            .all(Coefficient::is_zero)
        {
            return Err(
                FamilyPresentationError::PhysicalMomentumHasNoLoopComponent { denominator },
            );
        }
        for (external, coefficient) in physical.momentum().external_shift().iter().enumerate() {
            validate_coefficient(
                family,
                coefficient,
                PresentationCoefficientLocation::PhysicalExternalShift {
                    denominator,
                    external,
                },
            )?;
        }
        validate_coefficient(
            family,
            physical.mass_squared(),
            PresentationCoefficientLocation::PhysicalMassSquared { denominator },
        )?;
    }
    Ok(())
}

fn validate_physical_arity(
    denominator: usize,
    momentum: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), FamilyPresentationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(FamilyPresentationError::WrongPhysicalMomentumArity {
            denominator,
            momentum,
            expected,
            actual,
        })
    }
}

fn validate_routing(
    family: &IntegralFamily,
    routing: &MomentumRouting,
) -> Result<Option<Coefficient>, FamilyPresentationError> {
    validate_routing_labels(
        routing.source_loop_order(),
        routing.source_external_order(),
        family,
    )?;
    validate_matrix(
        routing.loop_linear(),
        family.loop_count(),
        family.loop_count(),
        "loop-linear",
        family,
        |row, column| PresentationCoefficientLocation::RoutingLoopLinear { row, column },
    )?;
    validate_matrix(
        routing.loop_external(),
        family.loop_count(),
        family.external_count(),
        "loop-external",
        family,
        |row, column| PresentationCoefficientLocation::RoutingLoopExternal { row, column },
    )?;
    validate_matrix(
        routing.external_linear(),
        family.external_count(),
        family.external_count(),
        "external-linear",
        family,
        |row, column| PresentationCoefficientLocation::RoutingExternalLinear { row, column },
    )?;

    let (loop_determinant, _) = determinant_of_coefficient_matrix(
        family.coefficient_context(),
        routing.loop_linear(),
        super::super::exact::symbolica_matrix_limits(family.limits),
    )
    .map_err(map_matrix_error)?;
    let one = family.coefficient_context().one();
    let minus_one = family.coefficient_context().integer(-1);
    if !coefficients_equal(family, &loop_determinant, &one)?
        && !coefficients_equal(family, &loop_determinant, &minus_one)?
    {
        return Err(FamilyPresentationError::NonUnimodularLoopRouting {
            determinant: loop_determinant,
        });
    }

    let external_determinant = if family.external_count() > 0 {
        let (external_determinant, _) = determinant_of_coefficient_matrix(
            family.coefficient_context(),
            routing.external_linear(),
            super::super::exact::symbolica_matrix_limits(family.limits),
        )
        .map_err(map_matrix_error)?;
        if external_determinant.is_zero() {
            return Err(FamilyPresentationError::SingularExternalRouting);
        }
        Some(external_determinant)
    } else {
        None
    };
    Ok(external_determinant)
}

fn validate_routing_labels(
    loops: &[String],
    externals: &[String],
    family: &IntegralFamily,
) -> Result<(), FamilyPresentationError> {
    validate_order_count("loop", loops.len(), family.loop_count())?;
    validate_order_count("external", externals.len(), family.external_count())?;
    for (momentum, labels) in [("loop", loops), ("external", externals)] {
        for (index, label) in labels.iter().enumerate() {
            if label.is_empty() {
                return Err(FamilyPresentationError::EmptyRoutingLabel { momentum, index });
            }
            if labels[..index].contains(label) {
                return Err(FamilyPresentationError::DuplicateRoutingLabel {
                    momentum,
                    label: try_copy_string(label, "duplicate presentation routing label")?,
                });
            }
        }
    }
    if let Some(label) = loops.iter().find(|label| externals.contains(label)) {
        return Err(FamilyPresentationError::RoutingLabelOverlap {
            label: try_copy_string(label, "overlapping presentation routing label")?,
        });
    }
    Ok(())
}

fn validate_order_count(
    momentum: &'static str,
    actual: usize,
    expected: usize,
) -> Result<(), FamilyPresentationError> {
    if actual == expected {
        Ok(())
    } else {
        Err(FamilyPresentationError::WrongRoutingOrderCount {
            momentum,
            expected,
            actual,
        })
    }
}

fn validate_matrix(
    matrix: &[Vec<Coefficient>],
    expected_rows: usize,
    expected_columns: usize,
    name: &'static str,
    family: &IntegralFamily,
    location: impl Fn(usize, usize) -> PresentationCoefficientLocation,
) -> Result<(), FamilyPresentationError> {
    if matrix.len() != expected_rows {
        return Err(FamilyPresentationError::WrongRoutingRowCount {
            matrix: name,
            expected: expected_rows,
            actual: matrix.len(),
        });
    }
    for (row, values) in matrix.iter().enumerate() {
        if values.len() != expected_columns {
            return Err(FamilyPresentationError::WrongRoutingColumnCount {
                matrix: name,
                row,
                expected: expected_columns,
                actual: values.len(),
            });
        }
        for (column, coefficient) in values.iter().enumerate() {
            validate_coefficient(family, coefficient, location(row, column))?;
        }
    }
    Ok(())
}

fn validate_common_scale(
    family: &IntegralFamily,
    roles: &[DenominatorRole],
    common_scale: Option<&CommonMassScale>,
) -> Result<(), FamilyPresentationError> {
    let Some(common_scale) = common_scale else {
        return Ok(());
    };
    validate_coefficient(
        family,
        common_scale.scale_squared(),
        PresentationCoefficientLocation::CommonMassScaleSquared,
    )?;
    if common_scale.scale_squared().is_zero() {
        return Err(FamilyPresentationError::ZeroCommonMassScale);
    }
    let mut physical_count = 0usize;
    let mut massive_count = 0usize;
    for (denominator, role) in roles.iter().enumerate() {
        let Some(physical) = role.physical() else {
            continue;
        };
        physical_count += 1;
        if physical.mass_squared().is_zero() {
            continue;
        }
        if !coefficients_equal(
            family,
            physical.mass_squared(),
            common_scale.scale_squared(),
        )? {
            return Err(FamilyPresentationError::PhysicalMassOutsideCommonScale { denominator });
        }
        massive_count += 1;
    }
    if physical_count == 0 {
        return Err(FamilyPresentationError::CommonMassScaleWithoutPhysicalDenominators);
    }
    if massive_count == 0 {
        return Err(FamilyPresentationError::CommonMassScaleUnused);
    }
    Ok(())
}

pub(super) fn validate_coefficient(
    family: &IntegralFamily,
    coefficient: &Coefficient,
    location: PresentationCoefficientLocation,
) -> Result<(), FamilyPresentationError> {
    family
        .coefficient_context()
        .validate_with_limits(coefficient, family.limits.exact_algebra)
        .map_err(|error| FamilyPresentationError::InvalidCoefficient { location, error })
}

pub(super) fn coefficients_equal(
    family: &IntegralFamily,
    left: &Coefficient,
    right: &Coefficient,
) -> Result<bool, FamilyPresentationError> {
    if left == right {
        return Ok(true);
    }
    Ok(family
        .coefficient_context()
        .try_sub(left, right, family.limits.exact_algebra)?
        .is_zero())
}

fn map_matrix_error(error: SymbolicaCoefficientMatrixError) -> FamilyPresentationError {
    match error {
        SymbolicaCoefficientMatrixError::ResourceLimit {
            resource,
            requested,
            limit,
        } => FamilyPresentationError::ResourceLimit {
            resource,
            requested,
            limit,
        },
        SymbolicaCoefficientMatrixError::ResourceCountOverflow { resource } => {
            FamilyPresentationError::ResourceCountOverflow { resource }
        }
        SymbolicaCoefficientMatrixError::AllocationFailure {
            resource,
            requested,
        } => FamilyPresentationError::AllocationFailure {
            resource,
            requested,
        },
        SymbolicaCoefficientMatrixError::InvalidCoefficient { error, .. }
        | SymbolicaCoefficientMatrixError::ExactAlgebra(error) => {
            FamilyPresentationError::ExactAlgebra(error)
        }
        other => FamilyPresentationError::RoutingMatrixFailure {
            detail: other.to_string(),
        },
    }
}

fn try_copy_string(
    source: &str,
    resource: &'static str,
) -> Result<String, FamilyPresentationError> {
    let mut copy = String::new();
    copy.try_reserve_exact(source.len()).map_err(|_| {
        FamilyPresentationError::AllocationFailure {
            resource,
            requested: source.len(),
        }
    })?;
    copy.push_str(source);
    Ok(copy)
}
