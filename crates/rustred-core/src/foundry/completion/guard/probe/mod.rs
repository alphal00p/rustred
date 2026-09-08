//! Exact, sample-bound admission for decorated coefficient-guard strata.
//!
//! A stratum branch is first decided after exact integer-index specialization
//! over generic base parameters. A nonzero branch is then checked again at the
//! concrete numeric base sample used by modular discovery. These logically
//! distinct checks prevent an accidental base root or an unlucky prime from
//! being mistaken for exact Zero-branch authority.

mod build;
mod error;
mod limits;
mod model;

pub(crate) use error::ExactGuardProbeError;
pub(crate) use limits::{ExactGuardPredicateCatalogLimits, ExactGuardProbeLimits};
pub(crate) use model::{ExactGuardPredicateCatalog, ExactGuardProbeWitness};

#[cfg(test)]
mod tests;
