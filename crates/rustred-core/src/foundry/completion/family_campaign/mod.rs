//! Topology-neutral planning of the sector obligations of one complete family.
//!
//! A physical graph matched by a caller is an integral in a complete affine
//! family, not a second family definition.  This module makes that distinction
//! structural: [`CompletePhysicalContractionGoal`] consumes the roles already
//! authenticated by [`crate::family::presentation::FamilyPresentation`], then
//! [`CompletePhysicalContractionPlan`] enumerates the complete physical
//! contraction downset and quotients it by an authenticated family symmetry.
//!
//! The resulting plan is suitable as the coverage authority for later staged
//! publication.  It is not closure evidence: every required sector still has
//! to be discharged by exact terminal or executable-owner authority.

mod error;
mod limits;
mod model;
mod plan;

pub(crate) use error::FamilyCoverageError;
pub(crate) use limits::FamilyCoverageLimits;
pub(crate) use model::{
    CompletePhysicalContractionGoal, CompletePhysicalContractionPlan, RequiredSectorOrbit,
};

#[cfg(test)]
mod tests;
