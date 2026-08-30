//! Exact generated singleton on the complementary mixed-dot orbit.
//!
//! A complete depth-three same-sector diamond supplies all 84 translations
//! of the nine ordinary K6 rows.  Exact elimination lowers the canonical
//! corner `J(0,1,2,3,2,0)`.  This cell deliberately fixes the whole scalar
//! four-line corner: it does not own either neighboring point of the
//! structural `J(0,1,2,N,2,0)` ray.

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::RuleCell;
use crate::foundry::parametric::ParametricRule;
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator};

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::support::complete_ordinary_sources;
use super::FOUR_LINE_SECTOR;
use super::corner::{
    derive_exact_corner_cell, derive_same_sector_candidate, project_complete_exact_corner_sources,
};

pub(super) const COMPLEMENTARY_MIXED_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 2, 1, 0];
const SEARCH_DEPTH: usize = 3;

pub(super) struct ComplementaryMixedDotCell {
    pub(super) context: IndexedCoefficientContext,
    pub(super) cell: RuleCell,
}

/// Derive the depth-three discovery cell from complete generated sources.
pub(super) fn derive_complementary_mixed_dot_cell()
-> Result<ComplementaryMixedDotCell, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?;
    let sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    let cell =
        derive_exact_corner_cell(&generator, sources, &COMPLEMENTARY_MIXED_DOT_TARGET_SHIFT)?;
    let context = generator.context().clone();
    drop(generator);
    Ok(ComplementaryMixedDotCell { context, cell })
}

/// Re-run one complete bounded search for the minimal-depth typed evidence.
pub(super) fn derive_complementary_mixed_dot_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    derive_same_sector_candidate(&COMPLEMENTARY_MIXED_DOT_TARGET_SHIFT, depth)
}

pub(super) const fn complementary_mixed_dot_search_depth() -> usize {
    SEARCH_DEPTH
}
