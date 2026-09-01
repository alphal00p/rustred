//! Shared, topology-neutral combinatorics for lattice-simplex proposal plans.
//!
//! This module contains no CAS, topology, sector, cover, or closure logic.  It
//! only counts and enumerates complete weak-composition simplices and finite
//! Cartesian assignments.  Boundary-subset unranking remains local to the
//! boundary planner because it is not part of an interior simplex design.

mod combinatorics;
mod error;

pub(super) use combinatorics::{
    checked_binomial, try_apply_finite_assignment, try_build_simplex_offsets,
    try_finite_assignment_count, try_simplex_sample_count,
};
pub(super) use error::SimplexSupportError;
