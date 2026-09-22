//! Ordered symbolic RHS inspection, not recursive closure or source generation.
mod algebra;
mod engine;
mod geometry;
mod model;
pub use model::*;
#[cfg(test)]
mod tests;
