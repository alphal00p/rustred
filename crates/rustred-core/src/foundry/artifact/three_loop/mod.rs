//! Test-only pressure fixture for the Stage 1 `K = 6` closure frontier.
//!
//! This module authenticates the family, source, symmetry, and sector manifests
//! needed by discovery. It also freezes a revision-stamped Vakint class/routing
//! snapshot and validates that snapshot internally; live cross-repository
//! matching remains a separate integration gate. The module deliberately does
//! not expose a [`super::ClosedArtifact`] until the rule fixed point is closed.

#[cfg(test)]
mod cells;
mod family;
mod manifest;
mod symmetry;
mod tests;

pub(crate) use family::canonical_family;
pub(crate) use symmetry::canonical_s4;
