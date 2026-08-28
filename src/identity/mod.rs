//! Typed identity rows and their exceptional-domain conditions.

mod condition;
mod row;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition, SpecializedNonZeroCondition,
};
pub use row::RowId;

pub(crate) use condition::{
    insert_parametric_condition, insert_specialized_condition,
    specialize_coefficient_with_condition,
};
