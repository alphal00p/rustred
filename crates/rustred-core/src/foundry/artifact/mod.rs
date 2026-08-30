//! Immutable, proof-bearing owners for completely discharged foundry output.
//!
//! The installed owner is generic in arity and topology. Complete verifiers
//! are registered for the generated unit-mass `K = 1` tadpole and `K = 3`
//! sunset families; a successful rule for another family is not thereby a
//! [`ClosedArtifact`].

mod error;
mod factorization;
mod install;
mod model;
mod one_loop;
mod persistence;
mod two_loop;

pub use error::{ArtifactError, ArtifactPersistenceError};
pub use factorization::{FactorizationFactor, FactorizationRule, UnimodularLoopBasis};
pub use model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
    ZeroSectorTerminal, ZeroTerminalProof,
};
pub use one_loop::derive_one_loop_unit_mass_tadpole;
pub use persistence::{ArtifactEncodingLimits, ArtifactLoadLimits};
pub use two_loop::derive_two_loop_unit_mass_sunset;

#[cfg(test)]
#[path = "tests/one_loop.rs"]
mod one_loop_tests;
#[cfg(test)]
#[path = "tests/persistence.rs"]
mod persistence_tests;
#[cfg(test)]
#[path = "tests/two_loop.rs"]
mod two_loop_tests;
