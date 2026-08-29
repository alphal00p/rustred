//! Typed parametric identity rows, sparse relations, and exceptional domains.

mod condition;
mod generator;
mod relation;
mod row;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition,
};
pub use generator::{
    CompletedIbpSourceRows, IbpSourceRow, ParametricIbpConfig, ParametricIbpError,
    ParametricIbpGenerator, PreparedIbpSourceBatch, PreparedLorentzInvarianceBatch,
};
pub use relation::{IndexShift, ParametricRelation, ParametricRelationError, RelationLimits};
pub use row::RowId;
