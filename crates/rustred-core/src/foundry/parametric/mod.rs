//! Exact parametric row derivation on one representable sector interior.
//!
//! This module eliminates generated identities directly over the authenticated
//! indexed field `K(n)`. A successful value proves its guarded source-row
//! replay, uniform structural descent, and agreement at one concrete anchor.
//! It deliberately makes no claim of exceptional-domain coverage or closure.

mod anchor;
mod derive;
mod error;
mod limits;
mod model;
mod prepare;
mod replay;
mod sparse;

pub use derive::derive_sector_interior_rule;
pub use error::ParametricRuleError;
pub use limits::ParametricRuleLimits;
pub use model::{
    AnchorAgreement, ParametricExactReplayWitness, ParametricGuardOrigin, ParametricNonZeroGuard,
    ParametricReducerPivotGuard, ParametricRule, ParametricRuleTerm,
    ParametricSourceRowContribution,
};

#[cfg(test)]
mod tests;
