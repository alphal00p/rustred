//! Fresh-frame rematerialization of one compact streaming support.
//!
//! A streaming hit is proposal evidence only. This boundary retains only its
//! canonical translated-source requests, rebuilds an independent exact frame,
//! repeats the modular target query from the original integer probe, and only
//! then enters exact lift and full source replay.

mod authority;
mod error;
mod limits;
mod model;
mod run;

pub(crate) use error::SpiredCompactLiftError;
pub(crate) use limits::SpiredCompactLiftLimits;
pub(crate) use model::{SpiredCompactLift, SpiredReplayedCompactLift};
pub(crate) use run::try_lift_spired_compact_support;

pub(crate) use authority::{SpiredRuleCellAuthorityError, try_promote_spired_replayed_rule_cell};

#[cfg(test)]
mod tests;
