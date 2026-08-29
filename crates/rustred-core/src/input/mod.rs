//! Topology-neutral Symbolica input compilation and exact family lowering.
//!
//! Every application transport converges on [`Project`]. The compiler owns the
//! untrusted expression boundary; transport schemas and metadata remain in the
//! application crate.

mod affine;
mod canonical;
mod compact;
mod compiler;
mod error;
mod gram;
mod limits;
mod lower;
mod model;
mod normalize;
mod parse;
mod request;
mod symbols;

#[cfg(test)]
mod tests;

pub use compact::COMPACT_SCHEMA;
pub use compiler::Compiler;
pub use error::{Error, LoweringError};
pub use limits::{Limits, LoweringLimits};
pub use model::{LoweredDenominator, LoweredProject, ParameterSource, Project, Propagator, Target};
pub use request::{
    AtomGramEntry, AtomProject, AtomPropagator, TextGramEntry, TextProject, TextPropagator,
};

pub use affine::{
    CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError,
    SymbolicaAffineDenominatorLimits,
};
