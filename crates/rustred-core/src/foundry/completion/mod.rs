//! Exact geometric state for sector-wide rule completion.
//!
//! A fixed sector is charted by nonnegative coordinates: active powers use
//! `x_i = n_i - 1`, while inactive powers use `x_i = -n_i`.  A symbolic rule
//! whose leading coordinate is nonnegative covers an upward orthant on the
//! current domain stratum.  [`LeadingIdeal`] keeps the minimal leading
//! antichain, and [`UncoveredPartition`] represents its complement exactly as
//! disjoint boxes, including unbounded axes and faces.
//!
//! This bounded E0 prototype is test-only until measured K6 evidence justifies
//! promoting it into the production foundry.  It owns lattice geometry only.
//! Coefficient guards, Symbolica elimination, source replay, strict descent,
//! and exceptional algebraic strata remain separate proof obligations.
//! [`SectorChart`] maps only the `i64` carrier.  No carrier endpoint is treated
//! as mathematical infinity without a separate asymptotic-extension witness.

mod chart;
mod coverage;
mod error;
pub(crate) mod frame;
mod limits;
mod model;
mod region;

pub(crate) use chart::SectorChart;
pub(crate) use coverage::{BoxCover, LeadingIdeal};
pub(crate) use error::CompletionGeometryError;
pub(crate) use limits::CompletionGeometryLimits;
pub(crate) use model::{LatticeBox, LatticeCardinality, LatticePoint, UncoveredPartition};
pub(crate) use region::{GuardBlindCarrierRegion, OuterPowerDirection};

#[cfg(test)]
mod tests;
