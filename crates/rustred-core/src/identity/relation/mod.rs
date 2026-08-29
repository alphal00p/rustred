//! Typed sparse relations on the parametric integral lattice.

mod error;
mod index;
mod limits;
mod model;
mod operations;

pub use error::ParametricRelationError;
pub use index::IndexShift;
pub use limits::RelationLimits;
pub use model::ParametricRelation;

pub(in crate::identity) use index::IndexSpace;

#[cfg(test)]
mod tests;
