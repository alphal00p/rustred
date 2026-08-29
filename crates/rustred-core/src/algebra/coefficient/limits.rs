/// Largest exponent representable by RustRed's Symbolica coefficient domain.
///
/// Symbolica's polynomial arithmetic panics when an operation would overflow
/// its exponent type. Analytic reducers use this ceiling to preflight their
/// caller-controlled formula degrees before constructing coefficients.
pub const SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT: u16 = u16::MAX;

/// Resource limits for exact rational-polynomial arithmetic.
///
/// These limits are checked before entering Symbolica operations that add
/// polynomial exponents. This is essential because Symbolica deliberately
/// panics when its `u16` exponent representation overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactAlgebraLimits {
    /// Largest exponent admitted by RustRed's checked representation boundary.
    pub max_exponent: u16,
    /// Largest authenticated retained sparse part.
    ///
    /// A conservative native-output envelope is a separate operation-local
    /// limit. In particular, direct polynomial multiplication may have a
    /// support envelope larger than its actual canonical result.
    pub max_polynomial_terms: usize,
    /// Sparse input-pair/sum admission bound for one checked operation.
    ///
    /// This is not a complete bound on Symbolica's internal GCD, quotient, or
    /// dense-multiplication scratch work. The vendored polynomial multiplier
    /// may scan a dense degree box (internally capped at `2^24` slots) even
    /// when the sparse Cartesian input has few pairs.
    pub max_term_operations: usize,
}

impl Default for ExactAlgebraLimits {
    fn default() -> Self {
        Self {
            max_exponent: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            max_polynomial_terms: 4_000_000,
            max_term_operations: 16_000_000,
        }
    }
}
