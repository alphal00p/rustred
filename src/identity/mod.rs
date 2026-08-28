//! Typed identity rows and their exceptional-domain conditions.

mod condition;
mod row;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition,
};
pub use row::RowId;

pub(crate) use condition::insert_parametric_condition;
