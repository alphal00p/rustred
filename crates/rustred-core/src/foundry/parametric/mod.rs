//! Exact parametric row derivation on one representable sector interior.
//!
//! This module eliminates generated identities directly over the authenticated
//! indexed field `K(n)`. A successful value proves its guarded source-row
//! replay, uniform structural descent, and agreement at one concrete anchor.
//! The target-directed sector-monotone API additionally constructs a maximal
//! representable parent-sector box and an exhaustive compact threshold
//! partition for every RHS shift. Pinched cells remain explicit lower-sector
//! dependencies. No API here claims exceptional-guard coverage, lower-rule
//! availability, or closure.

mod anchor;
mod boundary;
mod derive;
mod error;
mod limits;
mod model;
mod prepare;
mod replay;
mod sparse;

pub use boundary::{
    SectorMonotoneDependency, SectorMonotoneDependencyAtPoint, SectorMonotoneDependencyKind,
    SectorMonotoneTargetAdmission,
};
pub use derive::{
    derive_sector_interior_rule, derive_sector_interior_rule_for_target,
    derive_sector_monotone_rule_for_target,
};
pub use error::ParametricRuleError;
pub use limits::ParametricRuleLimits;
pub use model::{
    AnchorAgreement, ParametricExactReplayWitness, ParametricGuardOrigin, ParametricNonZeroGuard,
    ParametricReducerPivotGuard, ParametricRule, ParametricRuleTerm,
    ParametricSourceRowContribution,
};

#[cfg(test)]
mod tests;
