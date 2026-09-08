//! Exact promotion of one sealed compact replay into a `RuleCell` candidate.

mod error;
mod run;

pub(crate) use error::SpiredRuleCellAuthorityError;
pub(crate) use run::try_promote_spired_replayed_rule_cell;
