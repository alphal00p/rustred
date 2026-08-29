//! Exact, context-authenticated base-coefficient algebra backed by Symbolica.

mod context;
mod error;
mod limits;
mod model;
mod operations;
mod validation;

pub use context::CoefficientContext;
pub use error::{CoefficientContextError, ExactAlgebraError, ExactAlgebraOperation};
pub use limits::{ExactAlgebraLimits, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};
pub use model::{Coefficient, CoefficientPolynomial, CoefficientPolynomialPart};
pub(in crate::algebra) use operations::{
    trusted_coefficient_add_on_map, trusted_coefficient_mul_on_map, trusted_coefficient_neg_on_map,
    trusted_coefficient_sub_on_map,
};
pub(crate) use validation::{
    coefficient_clone_owned_retained_byte_bound, validate_coefficient_on_map,
    validate_polynomial_on_map,
};

#[cfg(test)]
mod tests;
