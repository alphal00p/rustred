//! Typed sparse relations on the parametric integral lattice.

mod builder;
mod error;
mod index;
mod limits;
mod model;
mod operations;
#[cfg(test)]
mod persistence;

pub use error::ParametricRelationError;
pub use index::IndexShift;
pub use limits::RelationLimits;
pub use model::ParametricRelation;

pub(in crate::identity) use builder::Builder;
pub(in crate::identity) use index::IndexSpace;

#[cfg(test)]
mod tests;
