//! Exact Symbolica-backed scalar algebra used throughout RustRed.

mod base;
pub(crate) mod matrix;

pub use base::{
    Coefficient, CoefficientContext, CoefficientContextError, CoefficientPolynomialPart,
    ExactAlgebraError, ExactAlgebraLimits, ExactAlgebraOperation,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};
pub(crate) use base::{
    checked_coefficient_add_on_map, checked_coefficient_mul_on_map, checked_coefficient_neg_on_map,
    checked_coefficient_sub_on_map, coefficient_clone_owned_retained_byte_bound,
    validate_coefficient_on_map, validate_polynomial_on_map,
};
