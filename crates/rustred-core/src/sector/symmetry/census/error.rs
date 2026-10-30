use std::fmt;

use crate::sector::Mask;
use crate::sector::symmetry::integral_transport;
use crate::sector::zero;

use super::super::DiscoveryError;

/// Typed census and routing failures; no partial census is returned.
/// Tolerated sector-local discovery failures are not errors; see
/// [`super::SectorSymmetryCensus::skips`].
#[derive(Debug)]
pub enum SectorSymmetryCensusError {
    Discovery(DiscoveryError),
    Verification(crate::sector::symmetry::Error),
    Transport(integral_transport::Error),
    Zero(zero::Error),
    Sector(crate::sector::Error),
    ExactAlgebra(String),
    /// The inherited Symbolica RSS/time budget was exhausted.
    Resource(symbolica::utils::resources::ResourceLimitError),
    /// A declared sector or integral key has the wrong denominator arity.
    WrongArity {
        expected: usize,
        actual: usize,
    },
    /// A declared sector has no active line.
    EmptySector,
    /// The integral's sector was neither declared to the census nor proved
    /// zero.
    UnknownSector(Mask),
    /// The integral's sector is in the census zero set: the integral
    /// vanishes and owns no route or automorphism rows.
    ZeroSector(Mask),
    /// Automorphism relations are owned by class representatives only.
    NotRepresentative(Mask),
    /// Subsector enumeration of this many active lines is not admitted.
    TooManySubsectors {
        active: usize,
        limit: usize,
    },
    /// The zero-sector analyzer belongs to a different family.
    ForeignAnalyzer,
    /// An explicitly enabled diagnostic normalization bound was exceeded.
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
}

impl fmt::Display for SectorSymmetryCensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discovery(error) => error.fmt(f),
            Self::Verification(error) => write!(f, "sector symmetry census map: {error}"),
            Self::Transport(error) => write!(f, "sector symmetry census transport: {error}"),
            Self::Zero(error) => write!(f, "sector symmetry census zero analysis: {error}"),
            Self::Sector(error) => write!(f, "sector symmetry census sector: {error}"),
            Self::ExactAlgebra(error) => write!(f, "sector symmetry census exact algebra: {error}"),
            Self::Resource(error) => write!(f, "sector symmetry census resources: {error}"),
            Self::WrongArity { expected, actual } => write!(
                f,
                "sector symmetry census arity {actual}, expected {expected}"
            ),
            Self::EmptySector => f.write_str("sector symmetry census sector has no active line"),
            Self::UnknownSector(mask) => {
                write!(f, "sector {mask} was not declared to the symmetry census")
            }
            Self::ZeroSector(mask) => {
                write!(f, "sector {mask} is in the symmetry census zero set")
            }
            Self::NotRepresentative(mask) => {
                write!(f, "sector {mask} is not a symmetry class representative")
            }
            Self::TooManySubsectors { active, limit } => write!(
                f,
                "subsector census of {active} active lines exceeds the limit of {limit}"
            ),
            Self::ForeignAnalyzer => {
                f.write_str("zero-sector analyzer belongs to a different family")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                f,
                "sector symmetry census {resource}: {requested} exceeds the limit of {limit}"
            ),
        }
    }
}

impl std::error::Error for SectorSymmetryCensusError {}

impl From<DiscoveryError> for SectorSymmetryCensusError {
    fn from(error: DiscoveryError) -> Self {
        Self::Discovery(error)
    }
}

impl From<crate::sector::symmetry::Error> for SectorSymmetryCensusError {
    fn from(error: crate::sector::symmetry::Error) -> Self {
        Self::Verification(error)
    }
}

impl From<integral_transport::Error> for SectorSymmetryCensusError {
    fn from(error: integral_transport::Error) -> Self {
        Self::Transport(error)
    }
}

/// Resource exhaustion keeps its type; other exact-algebra failures are
/// reported with their message.
impl From<crate::algebra::ExactAlgebraError> for SectorSymmetryCensusError {
    fn from(error: crate::algebra::ExactAlgebraError) -> Self {
        match error {
            crate::algebra::ExactAlgebraError::Resource(error) => Self::Resource(error),
            other => Self::ExactAlgebra(other.to_string()),
        }
    }
}

impl From<zero::Error> for SectorSymmetryCensusError {
    fn from(error: zero::Error) -> Self {
        Self::Zero(error)
    }
}

impl From<crate::sector::Error> for SectorSymmetryCensusError {
    fn from(error: crate::sector::Error) -> Self {
        Self::Sector(error)
    }
}
