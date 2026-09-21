//! Bounded concrete-key transport through an exactly verified momentum map.
//!
//! This is an integral identity, not a reducer, terminal declaration or closure
//! proof. Positive denominators must map by a unit-coefficient bijection of the
//! declared roots, at the same exact integration dimension. Only fixed
//! nonnegative numerator powers are expanded, using
//! the shared native Symbolica polynomial service. Routing between recursively
//! applied sector programs needs a separate well-founded ownership policy.

mod compile;
mod error;
mod model;
mod transport;

pub use crate::family::numerator_expansion::{
    MultiAffineNumeratorEndpoint as Endpoint, MultiAffineNumeratorExpansionError as ExpansionError,
    MultiAffineNumeratorExpansionLimits as ExpansionLimits,
};
pub use compile::compile;
pub use error::Error;
pub use model::{Prepared, TransportedIntegral};

#[cfg(test)]
mod tests;
