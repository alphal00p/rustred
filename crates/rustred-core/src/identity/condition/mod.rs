//! Deterministic provenance for identity nonzero conditions.

mod error;
mod limits;
mod source;
mod value;

pub use error::IdentityConditionError;
pub use limits::IdentityConditionLimits;
pub use source::IdentityConditionSource;
pub use value::ParametricNonZeroCondition;

pub(in crate::identity) use value::insert_parametric_condition;

#[cfg(test)]
mod tests;
