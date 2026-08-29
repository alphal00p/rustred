use std::fmt;

/// Failures while compiling an intrinsic permutation or admitting a concrete
/// restriction policy for its application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    ForeignFamily,
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    NonMonomial {
        source: usize,
    },
    NonUnitScale {
        source: usize,
        target: usize,
    },
    NonBijective {
        target: usize,
    },
    UnsupportedJacobian,
    PowerShiftMismatch {
        source: usize,
        target: usize,
    },
    WrongRestrictionArity {
        expected: usize,
        actual: usize,
    },
    CutMismatch {
        source: usize,
        target: usize,
    },
    PatternMismatch {
        source: usize,
        target: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignFamily => {
                formatter.write_str("affine map was verified for a different family")
            }
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} {resource} while compiling a family permutation"
            ),
            Self::NonMonomial { source } => write!(
                formatter,
                "source denominator {source} does not have a monomial image"
            ),
            Self::NonUnitScale { source, target } => write!(
                formatter,
                "source denominator {source} maps to {target} with a non-unit scale"
            ),
            Self::NonBijective { target } => write!(
                formatter,
                "target denominator {target} is not hit exactly once"
            ),
            Self::UnsupportedJacobian => {
                formatter.write_str("family permutation requires a unit loop Jacobian")
            }
            Self::PowerShiftMismatch { source, target } => write!(
                formatter,
                "power shift on source denominator {source} differs from target {target}"
            ),
            Self::WrongRestrictionArity { expected, actual } => write!(
                formatter,
                "permutation restrictions have arity {actual}; permutation expects {expected}"
            ),
            Self::CutMismatch { source, target } => write!(
                formatter,
                "cut membership on source denominator {source} differs from target {target}"
            ),
            Self::PatternMismatch { source, target } => write!(
                formatter,
                "sector-pattern slot on source denominator {source} differs from target {target}"
            ),
        }
    }
}

impl std::error::Error for Error {}

/// Arity failures while transporting powers into caller-owned storage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    WrongSourceArity { expected: usize, actual: usize },
    WrongTargetArity { expected: usize, actual: usize },
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSourceArity { expected, actual } => write!(
                formatter,
                "family permutation expects {expected} source powers, found {actual}"
            ),
            Self::WrongTargetArity { expected, actual } => write!(
                formatter,
                "family permutation expects {expected} target slots, found {actual}"
            ),
        }
    }
}

impl std::error::Error for TransportError {}
