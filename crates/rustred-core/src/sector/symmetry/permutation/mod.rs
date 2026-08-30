//! Compilation and application of exact internal family permutations.
//!
//! [`compile()`] accepts only an affine map already authenticated by
//! [`crate::sector::symmetry::verify()`]. It proves the intrinsic family action
//! once, independently of cuts and sector patterns. Callers check a concrete
//! [`Restrictions`] policy when selecting the permutation for application,
//! then reuse [`Verified::transport_into`] without allocating per integral.
//!
//! [`Restrictions`]: crate::sector::Restrictions

mod compile;
mod error;
mod model;
mod transport;

pub use compile::compile;
pub use error::{Error, TransportError};
pub use model::Verified;

#[cfg(test)]
mod tests;
