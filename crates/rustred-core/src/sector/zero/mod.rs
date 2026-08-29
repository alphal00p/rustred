//! Proof-bearing LiteRed-style zero-sector analysis.
//!
//! The production predicate is loop-count and topology independent. For an
//! effective sector face it extracts the exponent rows of `G = U + F` and
//! performs LiteRed's rank test exactly over `Q`. A zero result carries a
//! primitive integer right-kernel that is replayed before it is returned.

mod analysis;
mod domain;
mod error;
mod exponent;
mod limits;
mod model;
mod rank;

pub use analysis::ZeroSectorAnalyzer;
pub use domain::{ZeroSectorConditionSource, ZeroSectorDomain, ZeroSectorDomainCondition};
pub use error::ZeroSectorError;
pub use limits::{PowerShiftPolicy, ZeroSectorLimits};
pub use model::{
    FullColumnRankWitness, ZERO_SECTOR_CERTIFICATE_SCHEMA, ZeroSectorCertificate,
    ZeroSectorDecision, ZeroSectorResource,
};

#[cfg(test)]
mod tests;
