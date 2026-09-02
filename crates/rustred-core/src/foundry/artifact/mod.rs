//! Immutable, proof-bearing owners for completely discharged foundry output.
//!
//! The installed owner is generic in arity and topology. Complete verifiers
//! are registered for the generated unit-mass `K = 1` tadpole and `K = 3`
//! sunset families, and for a `K = 6` family only after its complete
//! authenticated six-sector wave chain publishes; a successful isolated rule
//! is not thereby a [`ClosedArtifact`].

mod error;
// This cold compiler is intentionally kept separate from the durable artifact
// and reducer until its routed-key integration has an authenticated owner.
mod factorization;
#[allow(dead_code, unused_imports)]
pub(crate) mod factorized_numerator_lift;
mod install;
mod model;
#[allow(dead_code)]
mod multi_affine_expansion;
mod one_loop;
mod persistence;
mod terminal;
mod three_loop;
mod two_loop;

pub use error::{ArtifactError, ArtifactPersistenceError};
pub use factorization::{
    FactorizationFactor, FactorizationMasterEmbedding, FactorizationRule, UnimodularLoopBasis,
};
pub use model::{
    ArtifactSchemaVersion, ArtifactValidationWitness, ClosedArtifact, CommonMassHomogeneityProof,
    ZeroSectorTerminal, ZeroTerminalProof,
};
pub use one_loop::derive_one_loop_unit_mass_tadpole;
pub use persistence::{ArtifactEncodingLimits, ArtifactLoadLimits};
pub use two_loop::derive_two_loop_unit_mass_sunset;

/// Consume a fully published K6 same-rank campaign and install its exact
/// owners as the same generic artifact type used by lower-loop families.
/// Search diagnostics and hint provenance are intentionally discarded before
/// this cold proof boundary.
pub fn install_published_k6_sector_waves(
    published: crate::foundry::campaign::K6PublishedSectorWaves,
) -> Result<ClosedArtifact, ArtifactError> {
    three_loop::install_published_sector_waves(published)
}

#[allow(unused_imports)]
pub(crate) use multi_affine_expansion::{
    MultiAffineNumeratorEndpoint, MultiAffineNumeratorExpansionError,
    MultiAffineNumeratorExpansionLimits, MultiAffineNumeratorFactor,
    try_expand_multi_affine_numerator,
};
pub(crate) use terminal::{ClosedTerminalAuthority, DeclaredMasterManifest};

pub(crate) use three_loop::{
    FULL_RANK_ORBITS, derive_k6_terminal_authority, derive_k6_terminal_authority_with_ordering,
};

pub(crate) use three_loop::canonical_family as canonical_three_loop_family;

#[cfg(test)]
pub(crate) use install::authenticate_k6_rule_cell_sources_for_test;
#[cfg(test)]
pub(crate) use three_loop::alphaloop_lhs_diagnostic::{
    MaterializedAlphaLoopLhsAnchor, certify_alpha_to_rust_map, materialize_alpha_loop_lhs_anchors,
    materialize_alpha_loop_lhs_anchors_with_ordering,
};
#[cfg(test)]
pub(crate) use three_loop::fresh_k6_terminal_authority_for_test;

#[cfg(test)]
#[path = "tests/one_loop.rs"]
mod one_loop_tests;
#[cfg(test)]
#[path = "tests/persistence.rs"]
mod persistence_tests;
#[cfg(test)]
#[path = "tests/two_loop.rs"]
mod two_loop_tests;
