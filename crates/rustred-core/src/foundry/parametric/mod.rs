//! Exact parametric row derivation on one representable sector interior.
//!
//! This module eliminates generated identities directly over the authenticated
//! indexed field `K(n)`. A successful value proves its guarded source-row
//! replay, uniform structural descent, and exact base-field replay at one
//! concrete anchor.
//! The target-directed sector-monotone API additionally constructs a maximal
//! representable parent-sector box and an exhaustive compact threshold
//! partition for every RHS shift. Pinched cells remain explicit lower-sector
//! dependencies. No API here claims exceptional-guard coverage, lower-rule
//! availability, or closure.

mod affine;
mod anchor;
mod boundary;
mod derive;
mod error;
mod evidence;
mod limits;
mod model;
mod original_domain;
mod prepare;
mod replay;
mod sparse;

#[cfg(test)]
pub(crate) use anchor::replay_rule_at_concrete_assignment;
#[allow(unused_imports)]
pub(crate) use anchor::verify_concrete_specialization_replay;

pub use affine::AffineApplicationDomain;
pub(crate) use affine::AffineDomainRestriction;
pub use boundary::{
    SectorMonotoneDependency, SectorMonotoneDependencyAtPoint, SectorMonotoneDependencyKind,
    SectorMonotoneTargetAdmission,
};
pub use derive::{
    derive_sector_interior_rule, derive_sector_interior_rule_for_target,
    derive_sector_monotone_rule_for_target,
};
pub use error::ParametricRuleError;
pub use evidence::{CombinedOriginalDomainEvidence, ParametricReplayEvidence};
pub use limits::ParametricRuleLimits;
pub use model::{
    ConcreteSpecializationReplayWitness, ParametricExactReplayWitness, ParametricGuardOrigin,
    ParametricNonZeroGuard, ParametricReducerPivotGuard, ParametricRule, ParametricRuleTerm,
    ParametricRuleTermDescent, ParametricSourceRowContribution,
};
pub(crate) use prepare::condition_source_index_cells;

#[cfg(test)]
pub(crate) mod tests;
