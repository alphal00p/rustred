//! Authenticated inputs for the Stage 1 `K = 6` closure frontier.
//!
//! This module authenticates the family, source, symmetry, and sector manifests
//! needed by discovery. It also freezes a revision-stamped Vakint class/routing
//! snapshot and validates that snapshot internally; live cross-repository
//! matching remains a separate integration gate. The module deliberately does
//! not expose a [`super::ClosedArtifact`] until the rule fixed point is closed.

#[cfg(test)]
pub(crate) mod alphaloop_lhs_diagnostic;
#[cfg(test)]
mod bootstrap_census;
#[cfg(test)]
mod cells;
#[cfg(test)]
mod closure_sweep;
mod factorization;
#[cfg(test)]
mod factorized_numerator_lift;
mod family;
#[cfg(test)]
mod isp_shell_probe;
mod manifest;
#[cfg(test)]
mod matcher_seed_portfolio;
#[cfg(test)]
mod momentum_rank;
#[cfg(test)]
mod ordering_portfolio;
mod publication;
#[cfg(test)]
mod rank_three_wave;
mod symmetry;
mod terminal_authority;
mod terminals;
#[cfg(test)]
mod tests;

pub(crate) use family::canonical_family;
pub(crate) use manifest::FULL_RANK_ORBITS;
pub(crate) use publication::{ALGORITHM_ID, install_published_sector_waves};
pub(crate) use symmetry::{canonical_s4, canonical_s4_with_ordering};
pub(crate) use terminal_authority::derive_k6_terminal_authority;
pub(crate) use terminal_authority::derive_k6_terminal_authority_with_ordering;
#[cfg(test)]
pub(crate) use terminals::exact_zero_sectors;

#[cfg(test)]
pub(crate) use terminal_authority::fresh_k6_terminal_authority_for_test;
#[cfg(test)]
pub(crate) use terminals::K6ReachabilityTerminals;
