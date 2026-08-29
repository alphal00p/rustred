//! Deterministic provenance for identity nonzero conditions.

mod error;
mod limits;
mod source;
mod value;

pub use error::IdentityConditionError;
pub use limits::IdentityConditionLimits;
pub use source::IdentityConditionSource;
pub use value::ParametricNonZeroCondition;

#[cfg(test)]
pub(in crate::identity) use value::borrowed_condition_deep_clone_counts;
pub(in crate::identity) use value::{
    insert_borrowed_parametric_condition, insert_parametric_condition,
};

#[cfg(test)]
mod tests;
