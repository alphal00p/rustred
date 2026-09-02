//! Cold assembly of a proof-bearing K6 wave campaign into the generic
//! closing-artifact candidate. Search policy and progress are deliberately
//! absent: only immutable published owners cross this boundary.

use crate::foundry::campaign::K6PublishedSectorWaves;
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};

use super::super::error::ArtifactError;
use super::super::install::{ClosingArtifactCandidate, install_published_k6};
use super::super::model::{ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof};
use super::derive_k6_terminal_authority_with_ordering;

pub(crate) const ALGORITHM_ID: &str = "rustred.generated.three-loop-unit-mass-vacuum-k6.v1";

pub(crate) fn install_published_sector_waves(
    published: K6PublishedSectorWaves,
) -> Result<ClosedArtifact, ArtifactError> {
    let waves = published.into_artifact_waves();
    let ordering = waves
        .first()
        .and_then(|wave| wave.predecessor().canonicalizer_ordering())
        .ok_or(ArtifactError::InvalidClosurePublication {
            detail: "published K6 waves do not retain an ordered root authority",
        })?;

    // Regenerate the canonical nine-row ordinary module independently of the
    // search transcript. This also provides an owned terminal payload with no
    // campaign configuration or external-hint provenance.
    let terminal_authority = derive_k6_terminal_authority_with_ordering(ordering)?;
    let parts = terminal_authority.into_artifact_parts();
    let generator =
        ParametricIbpGenerator::try_new_with_config(&parts.family, ParametricIbpConfig::default())?;
    let prepared = generator.prepare_ordinary_ibp()?;
    if prepared.len() != 9 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 family did not regenerate exactly nine ordinary IBP rows",
        });
    }
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    let completed = prepared.complete(rows)?;
    if !completed.is_complete_ordinary() || completed.source_row_count() != 9 {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "the K6 artifact source barrier is not the complete ordinary module",
        });
    }
    let source_relations = completed.into_relations();
    drop(generator);
    let factorized_product_programs = parts.factorized_product_programs;

    let candidate = ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id: ALGORITHM_ID,
        arity: parts.arity,
        ordering,
        // The K6 installer derives and replaces this placeholder exclusively
        // from every published layer's exact proof carrier.
        supported_root_power_bounds: Vec::new().into_boxed_slice(),
        family: parts.family,
        context: parts.context,
        source_relations,
        rules: Vec::new(),
        rule_cells: Vec::new(),
        canonicalizer: parts.canonicalizer,
        dependencies: parts.dependencies,
        factorization_rules: parts.factorization_rules,
        masters: parts.masters,
        zero_sectors: parts.zero_sectors,
        common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
    };
    install_published_k6(candidate, waves, factorized_product_programs)
}
