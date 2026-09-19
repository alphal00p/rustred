//! Exact, optional normalization of finite declared terminals.
//!
//! The first lane recognizes independent unit-mass tadpole products, using
//! Symbolica factorization only to propose integer loop momenta. Native matrix
//! inversion and the generic momentum-map verifier establish each equality.
//! An opt-in extension recognizes full-rank `L+1`-line momentum circuits.
//! Unsupported terminals are retained, and neither minimality nor closure is
//! claimed. Application/cache integration is deliberately separate.

mod corank_one;
mod model;
mod products;

pub use model::{
    ProductSkipReason, TerminalAliasError, TerminalAliasPlan, TerminalAliasStatistics,
    VerifiedTerminalAlias,
};

#[cfg(test)]
mod audit_tests;
#[cfg(test)]
mod tests;
