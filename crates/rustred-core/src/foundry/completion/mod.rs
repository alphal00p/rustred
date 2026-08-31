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
//! Its separate semantic-guard prototype compiles exact Symbolica coefficient
//! ideals into a bounded discovery DAG and binds generic predicates at one
//! exact index point. Physical-fibre sealing, source replay, strict descent,
//! and exceptional integer-locus ownership remain proof obligations before
//! either prototype gains closure authority.
//! [`SectorChart`] maps only the `i64` carrier.  No carrier endpoint is treated
//! as mathematical infinity without a separate asymptotic-extension witness.

mod chart;
mod coverage;
mod error;
pub(crate) mod frame;
pub(crate) mod guard;
mod limits;
mod model;
mod region;
pub(crate) mod source_discovery;
pub(crate) mod stratum;

pub(crate) use chart::SectorChart;
pub(crate) use coverage::{BoxCover, LeadingIdeal};
pub(crate) use error::CompletionGeometryError;
pub(crate) use limits::CompletionGeometryLimits;
pub(crate) use model::{LatticeBox, LatticeCardinality, LatticePoint, UncoveredPartition};
pub(crate) use region::{GuardBlindCarrierRegion, OuterPowerDirection};

#[cfg(test)]
mod tests;
