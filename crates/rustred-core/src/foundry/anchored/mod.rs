//! Exact row derivation at one concrete integral-index anchor.

mod derive;
mod error;
mod limits;
mod model;
mod prepare;
mod replay;
mod sparse;

pub use derive::{derive_strictly_descending_rule, derive_strictly_descending_rule_for_target};
pub use error::AnchoredRuleError;
pub use limits::AnchoredRuleLimits;
pub use model::{
    AnchoredNonZeroGuard, AnchoredRule, AnchoredRuleTerm, ExactReplayWitness, GuardOrigin,
    ReducerPivotGuard, SourceRowContribution,
};

#[cfg(test)]
mod tests;
