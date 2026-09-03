//! Bounded, proposal-only finite-field coefficient guidance.
//!
//! The arena retains a field-independent expression DAG and applies Ore
//! translations lazily.  Each [`ModularProbe`] owns an independent field,
//! point, accumulated-translation arena, and evaluation cache.  Its images
//! are scheduling evidence only: this module deliberately exposes no rule,
//! queue-discharge, exact-zero, or artifact-publication boundary.

mod arena;
mod error;
mod limits;
mod model;
mod probe;

use arena::ModularCoefficientDag;
use error::ModularGuideError;
use limits::ModularGuideLimits;
use model::{
    CoeffRef, ModularImage, ModularProbeCensus, ModularProbeIdentity, ModularZeroEvidence,
};
use probe::ModularProbe;

#[cfg(test)]
mod tests;
