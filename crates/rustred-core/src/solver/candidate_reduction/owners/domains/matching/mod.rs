//! Exact local ordered applicability in the existing coordinate/separable lane.
//! No source search, RHS evaluation, replay certification or recursive closure.
mod engine;
mod geometry;
mod guards;
mod model;
pub use model::*;
#[cfg(test)]
mod tests;
