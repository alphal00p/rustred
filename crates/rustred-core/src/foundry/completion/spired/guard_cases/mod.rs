//! Equality-only discovery cases induced by exact semantic guard zeros.
//!
//! Every required guard is zeroed independently on the parent coordinate
//! geometry. The resulting hyperplanes may overlap and contain no nonzero
//! prefix predicates: disjoint first-zero strata remain exclusively an owner-
//! compilation representation. This proposal-only bridge cannot create a
//! rule, owner, terminal, artifact, or closure claim.

mod error;
mod limits;
mod materialize;
mod model;

pub(crate) use error::SpiredCoordinateGuardCaseError;
pub(crate) use limits::SpiredCoordinateGuardCaseLimits;
pub(crate) use materialize::try_materialize_spired_coordinate_guard_cases;
pub(crate) use model::{
    SpiredCoordinateGuardCaseCensus, SpiredCoordinateGuardCaseIncomplete,
    SpiredCoordinateGuardCaseOutcome, SpiredCoordinateGuardCaseRejection,
    SpiredCoordinateGuardCases,
};

#[cfg(test)]
mod tests;
