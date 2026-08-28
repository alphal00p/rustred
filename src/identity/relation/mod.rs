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

// The parametric IBP generator still lives at the crate root. It needs the
// internal index-space constructor, checked shift addition, plus relation
// construction and mutation methods, so those seams remain `pub(crate)` until
// that generator moves under `identity`; all other relation internals are
// confined to this owner tree.
pub(crate) use index::IndexSpace;

#[cfg(test)]
mod tests;
