mod census;
mod error;
mod lift;
mod limits;
mod model;

pub(crate) use error::OrdinaryChartLiftError;
pub(crate) use lift::try_lift_completed_ordinary_sources;
pub(crate) use limits::OrdinaryChartLiftLimits;
pub(crate) use model::{LiftedOrdinarySource, LiftedOrdinarySourceBatch};

#[cfg(test)]
use lift::{build_lifted_source, preflight_relation};
#[cfg(test)]
#[path = "chart_lift_tests.rs"]
mod tests;
