//! Window-one fixed-point coordination for targeted SpIRed completion.
//!
//! This layer owns deterministic target chronology and live exact-ledger
//! mutation.  Algebraic target execution is deliberately injected through a
//! private runner seam until the resumable first-hit/exclusion portfolio is
//! available.  A completed bounded program is only incompleteness evidence;
//! compiler closure remains the sole closure authority.

mod error;
mod limits;
mod model;
mod run;
mod target;

pub(crate) use error::{SpiredFixedPointError, SpiredTargetRunnerError};
pub(crate) use limits::{
    SpiredFixedPointConfig, SpiredFixedPointLimits, SpiredTargetPortfolioBudget,
};
pub(crate) use model::{
    SpiredFixedPointCensus, SpiredFixedPointIncompleteReason, SpiredFixedPointReport,
    SpiredFixedPointStop, SpiredFixedPointTarget, SpiredFixedPointTargetRunner,
    SpiredTargetPortfolioCensus, SpiredTargetPortfolioDisposition,
    SpiredTargetPortfolioIncompleteReason, SpiredTargetPortfolioReport,
};
pub(crate) use run::try_drive_spired_fixed_point;

#[cfg(test)]
mod tests;
