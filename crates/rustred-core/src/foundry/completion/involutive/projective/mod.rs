//! Exact proposal-only projective Ore arithmetic over the authenticated
//! integer-polynomial ring.
//!
//! This module deliberately has no Janet-epoch, owner, terminal, or artifact
//! conversion.  It can clear one rational consequence, perform one exact
//! GCD-scaled pseudo-reduction, and expose a rational differential view.  A
//! future guided replay must still reconstruct the complete source witness
//! through the existing monic [`super::OreConsequence`] authority before any
//! result can cross a publication boundary. Intermediate replay may defer
//! augmented-content normalization, but a caller-owned cumulative budget and
//! a strictly descending selected-target cursor remain mandatory.

mod arithmetic;
mod error;
mod limits;
mod model;
mod polynomial;
mod replay;

#[cfg(test)]
mod tests;
