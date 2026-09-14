//! Compact, source-directed port of SpIRed's sector solver.
//!
//! This subsystem implements the reference search algorithm over Symbolica's
//! native polynomial and sparse-linear-algebra types. Family preparation and
//! artifact publication remain outside its per-row hot path. A sector search
//! result is not, by itself, a certified family-closing artifact.

mod case;
mod discovery;
mod error;
mod exception;
mod execution;
mod geometry;
mod index;
mod instantiate;
mod numeric;
mod precondition;
mod row;
mod search;
mod sector;
mod seed;
mod source;

pub use case::CoordinateCase;
pub use discovery::DiscoveryStats;
pub use error::SolverError;
pub use exception::{ExceptionError, ExceptionalConditions, extract_exceptions};
pub use execution::{
    SectorCompleted, SectorExecutionError, SectorExecutor, SectorExecutorBuildError,
    SectorScheduling,
};
pub use geometry::GeometryError;
pub use index::{Integral, IntegralOrder, Power, PowerError};
pub use numeric::{NumericResult, NumericStats};
pub use precondition::{precondition, precondition_with_variable_order};
pub use row::{ExactRow, PolynomialRow, Row, Term};
pub use search::{
    RuleCandidate, SearchOptions, SearchStats, SectorConfig, SectorSolver, SeedSource,
};
pub use sector::{
    SectorEvent, SectorRule, SectorSolution, SectorSolveError, SectorSolveOptions, SectorStats,
};
pub use seed::{Seed, Seeds};
pub use source::SourceSystem;

#[cfg(test)]
mod tests;
