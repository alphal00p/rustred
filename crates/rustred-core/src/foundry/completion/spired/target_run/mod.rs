//! One bounded scheduler-driven SpIRed target run.
//!
//! This private coordinator joins deterministic signed-L1 request scheduling,
//! direct streaming discovery, compact exact rematerialization, and ordinary
//! `RuleCell` promotion for one immutable case and probe. It grants no owner,
//! publication, terminal, or closure authority.

mod error;
mod limits;
mod model;
mod run;

pub(crate) use error::{SpiredTargetRunError, SpiredTargetRunErrorCause, SpiredTargetRunStage};
pub(crate) use limits::SpiredTargetRunLimits;
pub(crate) use model::{SpiredTargetRunCensus, SpiredTargetRunOutcome, SpiredTargetRunReport};
pub(crate) use run::{try_run_spired_guarded_target, try_run_spired_target};

#[cfg(test)]
mod tests;
