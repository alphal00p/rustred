//! Exact singleton recurrences for the two inequivalent mixed-dot orbits.
//!
//! Each owner starts from an independent complete depth-two same-sector
//! diamond: 28 generated translations of all nine ordinary K6 IBP rows.  The
//! residual projection specializes the exact scalar four-line corner and
//! retains its full setwise stabilizer.  These two isolated points are not a
//! mixed-dot ray and do not imply closure of the surrounding face.

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

pub(super) const ADJACENT_MIXED_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 1, 2, 0];
pub(super) const OPPOSITE_MIXED_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 0, 2, 0];
const SEARCH_DEPTH: usize = 2;

pub(super) struct MixedDotFourLineCells {
    pub(super) context: IndexedCoefficientContext,
    pub(super) adjacent: RuleCell,
    pub(super) opposite: RuleCell,
}

/// Derive independent exact singleton cells for the adjacent and opposite
/// placements of powers two and three on the scalar four-line corner.
pub(super) fn derive_mixed_dot_four_line_cells() -> Result<MixedDotFourLineCells, ArtifactError> {
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

    let adjacent_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    let adjacent = derive_exact_corner_cell(
        &generator,
        adjacent_sources,
        &ADJACENT_MIXED_DOT_TARGET_SHIFT,
    )?;

    // Keep a separately owned complete projection.  Sharing the first
    // projected batch would erase the provenance boundary between targets.
    let opposite_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    let opposite = derive_exact_corner_cell(
        &generator,
        opposite_sources,
        &OPPOSITE_MIXED_DOT_TARGET_SHIFT,
    )?;

    let context = generator.context().clone();
    drop(generator);
    Ok(MixedDotFourLineCells {
        context,
        adjacent,
        opposite,
    })
}

pub(super) fn derive_adjacent_mixed_dot_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    derive_same_sector_candidate(&ADJACENT_MIXED_DOT_TARGET_SHIFT, depth)
}

pub(super) fn derive_opposite_mixed_dot_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    derive_same_sector_candidate(&OPPOSITE_MIXED_DOT_TARGET_SHIFT, depth)
}

pub(super) const fn mixed_dot_search_depth() -> usize {
    SEARCH_DEPTH
}
