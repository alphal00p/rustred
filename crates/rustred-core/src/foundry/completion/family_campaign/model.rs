use std::sync::Arc;

use crate::family::IntegralKey;
use crate::family::presentation::FamilyPresentation;
use crate::sector::Mask;

use super::{FamilyCoverageError, FamilyCoverageLimits};

/// Publication-capable coverage goal for one complete family.
///
/// Construction derives the unique maximal physical sector: every physical
/// propagator in an authenticated [`FamilyPresentation`] is active and every
/// auxiliary coordinate is inactive. An arbitrary matcher-root list or raw
/// boolean role vector cannot be promoted into this type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletePhysicalContractionGoal {
    family_fingerprint: Arc<String>,
    physical_slot_count: usize,
    maximal_sector: Mask,
}

impl CompletePhysicalContractionGoal {
    pub(crate) fn try_new(presentation: &FamilyPresentation) -> Result<Self, FamilyCoverageError> {
        let physical_slot_count = presentation
            .denominator_roles()
            .iter()
            .filter(|role| role.physical().is_some())
            .count();
        if physical_slot_count == 0 {
            return Err(FamilyCoverageError::NoPhysicalPropagators);
        }
        let maximal_sector = Mask::try_new(
            presentation
                .denominator_roles()
                .iter()
                .map(|role| role.physical().is_some()),
        )?;
        Ok(Self {
            family_fingerprint: presentation.family().fingerprint_owner(),
            physical_slot_count,
            maximal_sector,
        })
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(super) fn family_fingerprint_owner(&self) -> Arc<String> {
        self.family_fingerprint.clone()
    }

    pub(crate) const fn physical_slot_count(&self) -> usize {
        self.physical_slot_count
    }

    pub(crate) const fn maximal_sector(&self) -> &Mask {
        &self.maximal_sector
    }

    pub(crate) fn try_plan(
        &self,
        canonicalizer: &crate::sector::symmetry::Canonicalizer,
        limits: FamilyCoverageLimits,
    ) -> Result<CompletePhysicalContractionPlan, FamilyCoverageError> {
        super::plan::try_plan_complete_downset(self, canonicalizer, limits)
    }
}

/// One exact symmetry orbit in the complete physical contraction downset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequiredSectorOrbit {
    sector: Mask,
    corner: IntegralKey,
    raw_sector_count: usize,
}

impl RequiredSectorOrbit {
    pub(super) fn new(sector: Mask, corner: IntegralKey, raw_sector_count: usize) -> Self {
        Self {
            sector,
            corner,
            raw_sector_count,
        }
    }

    pub(crate) const fn sector(&self) -> &Mask {
        &self.sector
    }

    pub(crate) const fn corner(&self) -> &IntegralKey {
        &self.corner
    }

    pub(crate) const fn raw_sector_count(&self) -> usize {
        self.raw_sector_count
    }
}

/// Deterministic exact quotient of a complete physical contraction downset.
///
/// This value proves coverage scope only.  It makes no terminal, owner, or
/// closure assertion and cannot itself be installed as an artifact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletePhysicalContractionPlan {
    family_fingerprint: Arc<String>,
    maximal_sector: Mask,
    raw_sector_count: usize,
    required_orbits: Box<[RequiredSectorOrbit]>,
}

impl CompletePhysicalContractionPlan {
    pub(super) fn new(
        family_fingerprint: Arc<String>,
        maximal_sector: Mask,
        raw_sector_count: usize,
        required_orbits: Vec<RequiredSectorOrbit>,
    ) -> Self {
        Self {
            family_fingerprint,
            maximal_sector,
            raw_sector_count,
            required_orbits: required_orbits.into_boxed_slice(),
        }
    }

    pub(crate) fn family_fingerprint(&self) -> &str {
        self.family_fingerprint.as_str()
    }

    pub(crate) const fn maximal_sector(&self) -> &Mask {
        &self.maximal_sector
    }

    pub(crate) const fn raw_sector_count(&self) -> usize {
        self.raw_sector_count
    }

    pub(crate) fn required_orbits(&self) -> &[RequiredSectorOrbit] {
        &self.required_orbits
    }
}
