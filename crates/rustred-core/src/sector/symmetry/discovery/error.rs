use std::fmt;

/// Typed failures of projector-graph proposal discovery.
///
/// Unsupported line geometry and rank-deficient supports are ordinary
/// outcomes ([`SupportSkipReason`]); only cancellation, foreign coefficients,
/// exact-algebra failures and violated native invariants are errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    /// The caller's cooperative cancellation callback returned `true`.
    Cancelled,
    /// A family coefficient does not belong to its declared context.
    ForeignCoefficient,
    /// Exact Symbolica algebra, matrix admission or map verification failed.
    ExactAlgebra(String),
    /// The family, a support or a native result violated a discovery gate.
    Invalid(&'static str),
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("sector symmetry preparation cancelled"),
            Self::ForeignCoefficient => {
                f.write_str("sector symmetry line coefficient is outside the family context")
            }
            Self::ExactAlgebra(error) => write!(f, "sector symmetry exact algebra: {error}"),
            Self::Invalid(reason) => write!(f, "invalid sector symmetry discovery: {reason}"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Why one denominator supplies no exact integer line momentum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LineGeometrySkip {
    /// The quadratic form contains loop-external scalar products.
    ExternalMomenta,
    /// A quadratic coefficient is not an integer constant.
    NonIntegerQuadratic,
    /// The quadratic form is not a positive multiple of one integer square.
    NotLinearSquare,
}

/// Why one declared support has no projector-graph proposal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SupportSkipReason {
    /// The active line momenta do not span every loop momentum.
    RankDeficient,
    /// The first active line without an exact integer square momentum.
    LineGeometry(LineGeometrySkip),
}

pub(super) fn exact(error: impl fmt::Display) -> DiscoveryError {
    DiscoveryError::ExactAlgebra(error.to_string())
}
