//! Proof-bearing LiteRed-style zero-sector analysis.
//!
//! The production predicate is loop-count and topology independent. For an
//! effective sector face it extracts the exponent rows of `G = U + F` and
//! performs LiteRed's rank test exactly over `Q`. A zero result carries a
//! primitive integer right-kernel that Symbolica replays before it is returned.

mod analysis;
mod domain;
mod error;
mod exponent;
mod limits;
mod model;
mod rank;

pub use analysis::Analyzer;
pub use domain::{ConditionSource, Domain, DomainCondition};
pub use error::Error;
pub use limits::Limits;
pub use model::{Certificate, Decision, FullColumnRank};

#[cfg(test)]
mod tests;
