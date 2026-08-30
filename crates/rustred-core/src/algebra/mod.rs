//! Exact Symbolica-backed scalar algebra used throughout RustRed.

mod coefficient;
pub mod indexed;
pub(crate) mod matrix;
mod symbol;

pub use coefficient::{
    Coefficient, CoefficientContext, CoefficientContextError, CoefficientPolynomial,
    CoefficientPolynomialPart, ExactAlgebraError, ExactAlgebraLimits, ExactAlgebraOperation,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};
pub(crate) use coefficient::{
    coefficient_clone_owned_retained_byte_bound, validate_coefficient_on_map,
    validate_polynomial_on_map,
};
#[cfg(test)]
pub(crate) use indexed::BaseCoefficientSystem;
pub use indexed::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficient, IndexedCoefficientContext,
    IndexedContextLimits, IndexedGuardLimits, IndexedPolynomial,
};
pub(crate) use symbol::is_exact_plain_symbol;
