use std::fmt;

use symbolica::prelude::{IntegerRing, MultivariatePolynomial, RationalPolynomial};

/// Exact rational functions in the kinematic parameters.
pub type Coefficient = RationalPolynomial<IntegerRing, u16>;

/// Exact integer polynomials in the authenticated coefficient variables.
///
/// This is the sole raw polynomial representation shared by RustRed's domain
/// layers. Domain-specific wrappers may add authentication or provenance, but
/// they must retain this algebra-owned representation internally.
pub type CoefficientPolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// The numerator or denominator of an exact coefficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoefficientPolynomialPart {
    Numerator,
    Denominator,
}

impl fmt::Display for CoefficientPolynomialPart {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numerator => formatter.write_str("numerator"),
            Self::Denominator => formatter.write_str("denominator"),
        }
    }
}
