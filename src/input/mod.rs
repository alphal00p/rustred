//! Topology-neutral Symbolica input compilation and exact family lowering.
//!
//! Every application transport converges on [`Project`]. The compiler owns the
//! untrusted expression boundary; transport schemas and metadata remain in the
//! application crate.

mod compiler;
mod error;
mod limits;
mod model;
mod request;

pub use compiler::Compiler;
pub use error::{Error, LoweringError};
pub use limits::{Limits, LoweringLimits, Stats};
pub use model::{
    LoweredDenominator, LoweredProject, ParameterSource, Project, ProjectSource, Propagator, Target,
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

/// Stable schema identifier for compact `I(...)` syntax.
pub const COMPACT_SCHEMA: &str = "rustred.symbolica-integral.v1";

/// Stable schema identifier for the current exactly lowered project payload.
pub const LOWERED_SCHEMA: &str = "rustred.lowered-symbolica-project.v1";
