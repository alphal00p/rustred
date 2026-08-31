//! Deterministic guarded owner covers built from replayed exact circuits.
//!
//! A semantic DAG owns an infinite orthant only when every abstract guard
//! branch selects an exact circuit, or exact locus analysis proves one ordered
//! candidate applicable throughout it, and every retained right-hand side is
//! rechecked to descend in the common sector order. Partial DAGs may still own
//! individually enumerated points after exact guard evaluation. Every other
//! finite point needs an explicit terminal declaration; an unbounded residue
//! or a reachable guard-incomplete ray remains a typed obstruction.

mod compile;
mod error;
mod limits;
mod model;
mod outer;

pub(crate) use error::ExactCircuitOwnerCoverError;
pub(crate) use limits::ExactCircuitOwnerCoverLimits;
pub(crate) use model::{
    ExactCircuitOwner, ExactCircuitOwnerCover, ExactCircuitOwnerId, ExactCircuitOwnerInput,
    ExactFinitePointOwner, ExactFiniteTerminalOwner, ExactOwnerCoverObstructionKind,
    ExactOwnerCoverSelection, ExactOwnerCoverStatus,
};
pub(crate) use outer::{ExactCircuitOuterExtensionError, ExactCircuitOuterExtensionWitness};

#[cfg(test)]
mod tests;
