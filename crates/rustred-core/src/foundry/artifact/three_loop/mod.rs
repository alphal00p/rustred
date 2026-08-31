//! Test-only pressure fixture for the Stage 1 `K = 6` closure frontier.
//!
//! This module authenticates the family, source, symmetry, and sector manifests
//! needed by discovery. It also freezes a revision-stamped Vakint class/routing
//! snapshot and validates that snapshot internally; live cross-repository
//! matching remains a separate integration gate. The module deliberately does
//! not expose a [`super::ClosedArtifact`] until the rule fixed point is closed.

#[cfg(test)]
mod bootstrap_census;
#[cfg(test)]
mod cells;
#[cfg(test)]
mod closure_sweep;
#[cfg(test)]
mod factorization;
mod family;
mod manifest;
#[cfg(test)]
mod momentum_rank;
mod symmetry;
#[cfg(test)]
mod terminal_authority;
#[cfg(test)]
mod terminals;
mod tests;

pub(crate) use family::canonical_family;
pub(crate) use symmetry::canonical_s4;
#[cfg(test)]
pub(crate) use terminal_authority::derive_k6_terminal_authority;
#[cfg(test)]
pub(crate) use terminals::{K6ReachabilityTerminals, exact_zero_sectors};
