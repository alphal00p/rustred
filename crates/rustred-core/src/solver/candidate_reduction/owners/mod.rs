//! Immutable, same-family candidate programs; not closure certificates.
mod model;
mod prepare;

pub use model::{
    CandidateOwnerContext, CandidateOwnerInput, CandidateOwnerPrograms, CandidateOwnerScope,
};

#[cfg(test)]
mod tests;
