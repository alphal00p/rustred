//! Bounded Symbolica expression boundary for topology-independent affine denominators.
//!
//! A denominator is supplied as an exact Symbolica `Atom`, together with an
//! ordered coefficient field, loop momenta, external momenta, and the complete
//! external Gram matrix. The compiler lowers Symbolica expressions directly to
//! affine-denominator coordinates without formatting or reparsing coefficients.
//! Production semantics are topology- and loop-count-independent.

const RUSTRED_NAMESPACE: &str = "rustred";
const SCALAR_PRODUCT_NAME: &str = "rustred::sp";
const CONSERVATIVE_GMP_CAPACITY_FACTOR: usize = 2;

mod budget;
mod compile;
mod construction;
mod error;
mod evaluate;
mod limits;
mod model;
mod normalize;
mod projection;
mod work;

#[cfg(test)]
mod tests;

pub use error::SymbolicaAffineDenominatorError;
pub use limits::SymbolicaAffineDenominatorLimits;
pub use model::CompiledSymbolicaAffineDenominator;
pub(super) use model::SymbolicaAffineDenominatorCompiler;
