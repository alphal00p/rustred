//! Resumable serial coordination of exact SpIReD equality cases.
//!
//! Logical worklist geometry is durable. Modular anchors are rebuilt from its
//! coordinate equalities and a finite explicit probe portfolio for each
//! attempt, and therefore never acquire terminal or closure authority.

mod error;
mod limits;
mod model;
mod probe;
mod run;

pub(crate) use error::{
    SpiredSerialDriverError, SpiredSerialProbeBuildError, SpiredSerialProbePortfolioError,
};
pub(crate) use limits::{
    SpiredSerialDriverConfig, SpiredSerialDriverLimits, SpiredSerialProbePortfolioLimits,
};
pub(crate) use model::{
    SpiredExistingTerminalAuthority, SpiredSerialCaseIncomplete, SpiredSerialCaseOutcome,
    SpiredSerialCaseReport, SpiredSerialDriverCensus, SpiredSerialDriverReport,
    SpiredSerialDriverStop, SpiredSerialFiniteResidualReason, SpiredSerialProbeCensus,
    SpiredSerialProbePortfolio, SpiredSerialResumePolicy,
};
pub(crate) use run::{
    try_drive_spired_serial_coordinate_cases, try_initialize_spired_serial_worklist,
};

#[cfg(test)]
mod tests;
