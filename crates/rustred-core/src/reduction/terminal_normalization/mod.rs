//! Exact, optional normalization of finite declared terminals.
//!
//! The first lane recognizes independent unit-mass tadpole products, using
//! Symbolica factorization only to propose integer loop momenta. Native matrix
//! inversion and the generic momentum-map verifier establish each equality.
//! An opt-in extension recognizes full-rank `L+1`-line momentum circuits.
//! A separate positive-power vacuum lane replays exact native U-polynomial
//! equality under power-preserving parameter permutations, without asserting
//! an integer momentum map.
//! A distinct weighted plan projects quadratic numerators with verified support
//! symmetries and exact native linear algebra, then binds positive outputs to
//! existing declared keys. Its native sidecar is replayed once at cold import.
//! Unsupported terminals are retained, and neither minimality nor closure is
//! claimed. Application/cache integration is deliberately separate.

mod corank_one;
mod model;
mod parametric;
mod products;
mod weighted;

pub use model::{
    ProductSkipReason, TerminalAliasError, TerminalAliasPlan, TerminalAliasStatistics,
    TerminalAliasWitness, VerifiedTerminalAlias,
};
pub use parametric::{VacuumParametricLimits, VerifiedVacuumParameterMap};
pub use weighted::{
    TerminalNormalizationError, TerminalNormalizationLimits, TerminalNormalizationPlan,
    TerminalNormalizationSkipReason, TerminalNormalizationStatistics, TerminalProjectionWitness,
    VerifiedNumeratorSupport,
};

#[cfg(test)]
mod audit_tests;
#[cfg(test)]
mod tests;
