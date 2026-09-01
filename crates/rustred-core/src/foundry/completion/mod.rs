//! Exact geometric state for sector-wide rule completion.
//!
//! A fixed sector is charted by nonnegative coordinates: active powers use
//! `x_i = n_i - 1`, while inactive powers use `x_i = -n_i`.  A symbolic rule
//! whose leading coordinate is nonnegative covers an upward orthant on the
//! current domain stratum.  [`LeadingIdeal`] keeps the minimal leading
//! antichain, and [`UncoveredPartition`] represents its complement exactly as
//! disjoint boxes, including unbounded axes and faces.
//!
//! The engine is crate-private production code during Stage 1.  Discovery
//! evidence never gains closure authority by existing: exact source replay,
//! strict descent, guard-stratum refinement, immutable lower-sector ownership,
//! and an independently compiled owner cover remain mandatory promotion
//! boundaries.
//! [`SectorChart`] maps only the `i64` carrier.  No carrier endpoint is treated
//! as mathematical infinity without a separate asymptotic-extension witness.

mod chart;
mod coverage;
mod error;
mod family_campaign;
pub(crate) mod frame;
pub(crate) mod guard;
mod limits;
mod model;
mod region;
pub(crate) mod source_discovery;
pub(crate) mod stratum;

pub(crate) use chart::SectorChart;
#[allow(unused_imports)] // Consumed by the staged owner-cover publisher.
pub(crate) use coverage::BoxCover;
pub(crate) use coverage::LeadingIdeal;
pub(crate) use error::CompletionGeometryError;
#[allow(unused_imports)] // First topology-generic family-closure planning slice.
pub(crate) use family_campaign::{
    CompletePhysicalContractionGoal, CompletePhysicalContractionPlan, FamilyCoverageError,
    FamilyCoverageLimits, RequiredSectorOrbit,
};
pub(crate) use limits::CompletionGeometryLimits;
pub(crate) use model::{LatticeBox, LatticeCardinality, LatticePoint, UncoveredPartition};
#[allow(unused_imports)] // Consumed by the staged owner-cover publisher.
pub(crate) use region::{GuardBlindCarrierRegion, OuterPowerDirection};

#[cfg(test)]
mod tests;
