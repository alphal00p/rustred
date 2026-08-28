//! Deterministic provenance for identity nonzero conditions.

mod error;
mod limits;
mod source;
mod value;

pub use error::IdentityConditionError;
pub use limits::IdentityConditionLimits;
pub use source::IdentityConditionSource;
pub use value::ParametricNonZeroCondition;

// These relation-construction helpers remain crate-visible while the relation
// implementation still lives at the crate root. Moving that implementation
// under `identity` will allow this boundary to narrow to the parent module.
pub(crate) use value::insert_parametric_condition;

#[cfg(test)]
mod tests;
