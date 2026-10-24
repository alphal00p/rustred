/// Largest exponent representable by RustRed's Symbolica coefficient domain.
///
/// Symbolica's polynomial arithmetic panics when an operation would overflow
/// its exponent type. Analytic reducers use this ceiling to preflight their
/// caller-controlled formula degrees before constructing coefficients.
pub const SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT: u16 = u16::MAX;

/// Optional diagnostic thresholds for exact rational-polynomial arithmetic.
///
/// With diagnostics enabled, thresholds are checked before operations that add
/// polynomial exponents. This is essential because Symbolica deliberately
/// panics when its `u16` exponent representation overflows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExactAlgebraLimits {
    /// Enforce count/work thresholds only for an explicitly requested diagnostic.
    pub diagnostics_enabled: bool,
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

impl ExactAlgebraLimits {
    /// Explicit trusted-computation policy without term/operation quotas.
    /// The coefficient exponent representation and checked resource arithmetic
    /// remain enforced. External RAM controls are still the caller's duty.
    pub const fn unbounded_resources() -> Self {
        Self {
            diagnostics_enabled: false,
            max_exponent: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            max_polynomial_terms: usize::MAX,
            max_term_operations: usize::MAX,
        }
    }
}

impl Default for ExactAlgebraLimits {
    fn default() -> Self {
        Self {
            diagnostics_enabled: false,
            max_exponent: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            max_polynomial_terms: 4_000_000,
            max_term_operations: 16_000_000,
        }
    }
}

impl ExactAlgebraLimits {
    /// Explicitly enable diagnostic count/work thresholds.
    pub fn with_diagnostics(mut self) -> Self {
        self.diagnostics_enabled = true;
        self
    }
    pub(crate) const fn exponent_limit(self) -> u16 {
        if self.diagnostics_enabled {
            self.max_exponent
        } else {
            u16::MAX
        }
    }
}

impl ExactAlgebraLimits {
    pub(crate) fn bounded_by(self, outer: Self) -> Self {
        match (self.diagnostics_enabled, outer.diagnostics_enabled) {
            (true, false) => self,
            (false, true) => outer,
            (false, false) => self,
            (true, true) => Self {
                diagnostics_enabled: true,
                max_exponent: self.max_exponent.min(outer.max_exponent),
                max_polynomial_terms: self.max_polynomial_terms.min(outer.max_polynomial_terms),
                max_term_operations: self.max_term_operations.min(outer.max_term_operations),
            },
        }
    }
}
