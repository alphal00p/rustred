//! Typed parametric identity rows, sparse relations, and exceptional domains.

mod condition;
mod relation;
mod row;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition,
};
pub use relation::{IndexShift, ParametricRelation, ParametricRelationError, RelationLimits};
pub use row::RowId;

// Temporary while the serial parametric IBP generator remains at crate root.
pub(crate) use relation::IndexSpace;
