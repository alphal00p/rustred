//! Lossless lowering of one live-plan-bound exact circuit into the existing
//! parametric rule representation.
//!
//! This boundary reconstructs and replays an identity only. It deliberately
//! creates no rule cell, owner, terminal, or closure authority.

mod compile;
mod error;
mod guards;
mod join;
mod limits;
mod model;
mod preflight;
mod replay;
mod resource;
mod source;

pub(crate) use compile::try_lower_exact_circuit;
pub(crate) use error::ExactCircuitLoweringError;
pub(crate) use limits::ExactCircuitLoweringLimits;
pub(crate) use model::{ExactCircuitLoweringSeal, LoweredExactCircuit};
