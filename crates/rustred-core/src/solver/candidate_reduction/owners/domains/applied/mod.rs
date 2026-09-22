//! Ordered symbolic RHS inspection, not recursive closure or source generation.
mod algebra;
mod engine;
mod geometry;
mod model;
pub(super) mod restriction;
pub(super) use engine::Budget;
pub(super) use geometry::{copy_box, fixed, rank_empty};
pub use model::*;
#[cfg(test)]
mod tests;
