//! Immutable, proof-bearing owners for completely discharged foundry output.
//!
//! The installed owner is generic in arity and topology. The only complete
//! closure verifier currently registered is the generated one-loop,
//! unit-common-mass vacuum preset; a successful rule for another family is
//! not thereby a [`ClosedArtifact`].

mod error;
mod install;
mod model;
mod one_loop;

pub use error::{ArtifactError, ArtifactPersistenceError};
pub use model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
    ZeroSectorTerminal, ZeroTerminalProof,
};
pub use one_loop::derive_one_loop_unit_mass_tadpole;

#[cfg(test)]
mod tests;
