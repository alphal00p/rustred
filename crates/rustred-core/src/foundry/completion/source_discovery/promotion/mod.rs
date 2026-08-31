//! Fail-closed promotion of one scheduler-owned exact replay into an
//! executable rule-cell candidate.
//!
//! The promoted value retains the exact epoch and circuit beside the lowered
//! `RuleCell`.  The cell is executable payload only; the epoch/circuit pair
//! remains the authority needed by semantic-DAG and owner-cover compilation.

mod admit;
mod error;
mod limits;
mod model;

pub(crate) use admit::{
    try_promote_replayed_rule_cell, try_promote_replayed_rule_cell_on_partition,
};
pub(crate) use error::ExactRuleCellPromotionError;
pub(crate) use limits::ExactRuleCellPromotionLimits;
pub(crate) use model::{
    AdmittedExactRuleCandidate, ExactRuleCellGuardObstruction, ExactRuleCellPromotionDisposition,
};

#[cfg(test)]
mod tests;
