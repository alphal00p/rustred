//! Proof-bearing optimized-lane admission.

use std::fmt;

use super::model::{FamilyPresentation, PhysicalPropagator};

/// Why the optimized single-scale vacuum lane cannot consume a presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SingleScaleVacuumIneligibility {
    NoPhysicalDenominators,
    MissingCommonMassScale,
    PhysicalExternalShift { denominator: usize, external: usize },
}

impl fmt::Display for SingleScaleVacuumIneligibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPhysicalDenominators => {
                formatter.write_str("the presentation has no physical denominators")
            }
            Self::MissingCommonMassScale => {
                formatter.write_str("the presentation has no authenticated common mass scale")
            }
            Self::PhysicalExternalShift {
                denominator,
                external,
            } => write!(
                formatter,
                "physical denominator {denominator} is shifted by external momentum {external}"
            ),
        }
    }
}

impl std::error::Error for SingleScaleVacuumIneligibility {}

/// Sealed evidence for common-scale physical denominators without external
/// shifts.
///
/// Its field is private and there is no public constructor.  Only exact
/// presentation authentication followed by semantic admission can mint it.
#[derive(Clone, Copy, Debug)]
pub struct SingleScaleVacuumEvidence<'presentation> {
    presentation: &'presentation FamilyPresentation,
}

impl<'presentation> SingleScaleVacuumEvidence<'presentation> {
    pub const fn presentation(&self) -> &'presentation FamilyPresentation {
        self.presentation
    }

    pub fn common_mass_scale(&self) -> &super::model::CommonMassScale {
        self.presentation
            .common_mass_scale()
            .expect("sealed vacuum evidence proves the common scale exists")
    }

    /// Exact guard that makes a symbolic common scale nonzero on the admitted
    /// generic domain.  It is also present in [`FamilyPresentation::domain`].
    pub fn common_mass_scale_nonzero_numerator(&self) -> &crate::algebra::CoefficientPolynomial {
        &self.common_mass_scale().scale_squared().numerator
    }

    /// Presentation-only coefficient-denominator and common-scale guards.
    /// Family-owned guards remain available through the contained family.
    pub const fn presentation_domain(&self) -> &super::model::PresentationDomain {
        self.presentation.domain()
    }

    pub fn physical_denominators(&self) -> impl Iterator<Item = (usize, &PhysicalPropagator)> {
        self.presentation
            .denominator_roles()
            .iter()
            .enumerate()
            .filter_map(|(denominator, role)| {
                role.physical().map(|physical| (denominator, physical))
            })
    }
}

impl FamilyPresentation {
    /// Prove eligibility for the optimized single-scale vacuum lane.
    ///
    /// External momenta may remain in the family for numerator spectators or
    /// auxiliary coordinates.  Only a nonzero external coefficient in a
    /// physical propagator momentum invalidates this proof.
    pub fn single_scale_vacuum_evidence(
        &self,
    ) -> Result<SingleScaleVacuumEvidence<'_>, SingleScaleVacuumIneligibility> {
        let mut physical_count = 0usize;
        for (denominator, role) in self.denominator_roles().iter().enumerate() {
            let Some(physical) = role.physical() else {
                continue;
            };
            physical_count += 1;
            if let Some((external, _)) = physical
                .momentum()
                .external_shift()
                .iter()
                .enumerate()
                .find(|(_, coefficient)| !coefficient.is_zero())
            {
                return Err(SingleScaleVacuumIneligibility::PhysicalExternalShift {
                    denominator,
                    external,
                });
            }
        }
        if physical_count == 0 {
            return Err(SingleScaleVacuumIneligibility::NoPhysicalDenominators);
        }
        if self.common_mass_scale().is_none() {
            return Err(SingleScaleVacuumIneligibility::MissingCommonMassScale);
        }
        Ok(SingleScaleVacuumEvidence { presentation: self })
    }
}
