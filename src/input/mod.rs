//! Topology-neutral Symbolica input compilation and exact family lowering.
//!
//! Every application transport converges on [`Project`]. The compiler owns the
//! untrusted expression boundary; transport schemas and metadata remain in the
//! application crate.

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
pub use limits::{Limits, LoweringLimits, Stats};
pub use model::{
    LOWERED_SCHEMA, LoweredDenominator, LoweredProject, ParameterSource, Project, ProjectSource,
    Propagator, Target,
};
pub use request::{
    AtomGramEntry, AtomProject, AtomPropagator, TextGramEntry, TextProject, TextPropagator,
};

// Step A keeps the affine implementation in its existing private root module.
// These are its sole public paths until the affine facade is made internal in
// the later pruning tranche.
pub use crate::symbolica_affine_denominator::{
    CompiledSymbolicaAffineDenominator, SymbolicaAffineDenominatorError,
    SymbolicaAffineDenominatorLimits, SymbolicaAffineDenominatorStats,
};
