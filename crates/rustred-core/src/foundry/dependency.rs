//! Bounded discovery of unresolved rule dependencies.
//!
//! This owner converts the compact sector-monotone evidence of one parametric
//! rule into a lazy, stable-ordinal stream of exact proper-subsector cells.
//! Every obligation retains its RHS coefficient and the rule guards whose
//! applicability still needs Boolean-domain refinement. A complete traversal
//! is a dependency-discovery result only: it is not a fixed point, closed
//! sector, selected master set, or publishable artifact.

mod error;
mod limits;
mod model;
mod plan;

pub use error::ParametricDependencyError;
pub use limits::ParametricDependencyLimits;
pub use model::{
    ParametricDependencyCursor, ParametricProperSubsectorObligation,
    ParametricProperSubsectorObligations,
};
pub use plan::ParametricProperSubsectorPlan;
