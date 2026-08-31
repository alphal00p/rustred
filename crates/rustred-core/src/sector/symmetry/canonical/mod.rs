//! Deterministic exact-orbit ownership for authenticated family symmetries.
//!
//! Candidate momentum maps remain outside this boundary. [`Canonicalizer`]
//! consumes only denominator permutations already authenticated through
//! [`crate::sector::symmetry::verify()`] and
//! [`crate::sector::symmetry::permutation::compile`], derives their complete
//! finite group, and selects the minimum existing persisted integral-order
//! key. Exact route and weak-order witnesses let a reducer canonicalize roots
//! and raw descending children before memoization without weakening its
//! termination proof.

mod action;
mod error;
mod model;
#[cfg(test)]
mod priority;

pub use action::Canonicalizer;
pub use error::Error;
pub use model::{
    Canonicalization, CanonicalizationLimits, DEFAULT_MAX_GENERATORS, DEFAULT_MAX_GROUP_ENTRIES,
    DEFAULT_MAX_GROUP_ORDER, DescendingCanonicalization, ExactOrbit, NoHarderWitness, OrbitImage,
    RoutingCoefficient, RoutingWitness,
};
#[cfg(test)]
pub(crate) use priority::{CoordinatePriorityActionLimits, CoordinatePriorityQuotient};

#[cfg(test)]
mod tests;
