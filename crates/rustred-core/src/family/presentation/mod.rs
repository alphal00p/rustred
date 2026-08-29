//! Authenticated semantic presentation of an exact integral family.
//!
//! A presentation adds the information that is deliberately absent from the
//! affine family engine: physical-propagator versus auxiliary-coordinate
//! roles, caller-attested momentum-routing metadata with exact structural and
//! invertibility checks, and metric, denominator-sign, and common-mass-scale
//! conventions. Physical rows and scale claims are replayed exactly;
//! source-side routing replay remains the topology matcher's responsibility.
//! Optimized services may consume only the sealed evidence minted here;
//! caller booleans and topology names are never evidence.

mod admission;
mod authentication;
mod domain;
mod error;
mod evidence;
mod limits;
mod model;
mod replay;

#[cfg(test)]
mod tests;

pub use error::{
    FamilyPresentationError, PresentationCoefficientLocation, PresentationDenominatorComponent,
};
pub use evidence::{SingleScaleVacuumEvidence, SingleScaleVacuumIneligibility};
pub use limits::FamilyPresentationLimits;
pub use model::{
    AlgebraicSign, AuxiliaryDenominator, CommonMassScale, DenominatorRole, FamilyConventions,
    FamilyPresentation, MetricConvention, MomentumCombination, MomentumRouting, PhysicalPropagator,
    PresentationConditionSource, PresentationDomain, PresentationNonZeroCondition,
    PropagatorConvention,
};
