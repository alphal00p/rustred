use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
    TranslatedSourceLimits,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::super::{canonical_family, canonical_s4};
use super::super::support::{complete_ordinary_sources, exact_zero_sectors};
use super::{CANONICAL_DOT_TARGET_SHIFT, FOUR_LINE_SECTOR, ZERO_SOURCE_SHIFT};

const BASE_CORNER: [i64; 6] = FOUR_LINE_SECTOR;
const ISOLATED_DOT_SOURCE_ORDINALS: [usize; 2] = [0, 3];
const OPPOSITE_PAIR_SOURCE_SHIFT: [i64; 6] = [0, 0, 1, 0, 0, 0];
const ADJACENT_PAIR_SEARCH_DEPTH: usize = 2;
pub(super) const ADJACENT_DOT_PAIR_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 1, 1, 0];
pub(super) const OPPOSITE_DOT_PAIR_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 0, 1, 0];

/// Derive the three exact singleton cells currently owned on the scalar
/// four-line corner.
///
/// The first lowers the isolated canonical dot excluded by the ordinary
/// positive-box recurrence. The second lowers the opposite two-dot orbit from
/// one complete nine-row translated ordinary-source layer. The third lowers
/// the adjacent two-dot orbit from the complete depth-two same-sector search
/// diamond: 28 translations and all nine ordinary rows at each translation.
/// All three specialize the whole base corner so its order-eight setwise
/// stabilizer can route equivalent edge decorations. Deeper dot and numerator
/// faces remain explicit closure obligations.
pub(super) fn derive_exceptional_four_line_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell, RuleCell), ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;

    let isolated_sources = project_exact_corner_sources(
        &generator,
        &completed,
        &ISOLATED_DOT_SOURCE_ORDINALS,
        &canonicalizer,
        &zero_sectors,
        ZERO_SOURCE_SHIFT,
    )?;
    let isolated =
        derive_exact_corner_cell(&generator, isolated_sources, &CANONICAL_DOT_TARGET_SHIFT)?;

    let opposite_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        [IntegralShift::try_new(OPPOSITE_PAIR_SOURCE_SHIFT)?],
    )?;
    let opposite = derive_exact_corner_cell(
        &generator,
        opposite_sources,
        &OPPOSITE_DOT_PAIR_TARGET_SHIFT,
    )?;

    let adjacent_search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(BASE_CORNER)?,
        ADJACENT_PAIR_SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?;
    let adjacent_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        adjacent_search.offsets().iter().cloned(),
    )?;
    let adjacent = derive_exact_corner_cell(
        &generator,
        adjacent_sources,
        &ADJACENT_DOT_PAIR_TARGET_SHIFT,
    )?;

    let context = generator.context().clone();
    drop(generator);
    Ok((context, isolated, opposite, adjacent))
}

/// Re-run one bounded complete same-sector diamond for the adjacent pair.
/// Depths below two retain concise typed witnesses for search minimality.
pub(super) fn derive_adjacent_same_sector_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _source_count) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(BASE_CORNER)?,
        depth,
        SectorSearchLimits::default(),
    )?;
    let sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    Ok(derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &BASE_CORNER,
        &ADJACENT_DOT_PAIR_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

pub(super) const fn adjacent_pair_search_depth() -> usize {
    ADJACENT_PAIR_SEARCH_DEPTH
}

pub(super) fn fixed_base_corner() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, BASE_CORNER[position]))
}

fn derive_exact_corner_cell(
    generator: &ParametricIbpGenerator<'_>,
    sources: SourceViewBatch,
    target_shift: &[i64; 6],
) -> Result<RuleCell, ArtifactError> {
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &BASE_CORNER,
        target_shift,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let application = SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        BASE_CORNER.map(|value| InteriorBounds::new(value, value)),
        rule.pivot().values(),
        &rhs,
    )?;
    Ok(RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed_base_corner(),
        [],
        RuleCellLimits::default(),
    )?)
}

fn project_exact_corner_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    ordinals: &[usize],
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    translation: [i64; 6],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        [IntegralShift::try_new(translation)?],
        TranslatedSourceLimits::default(),
    )?;
    Ok(SourceViewBatch::try_project_residual(
        translated,
        ordinals,
        generator.context(),
        singleton_domain()?,
        fixed_base_corner(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn project_complete_exact_corner_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    translations: impl IntoIterator<Item = IntegralShift>,
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        translations,
        TranslatedSourceLimits::default(),
    )?;
    Ok(SourceViewBatch::try_project_complete_residual(
        translated,
        generator.context(),
        singleton_domain()?,
        fixed_base_corner(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn singleton_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        BASE_CORNER.map(|value| InteriorBounds::new(value, value)),
    )?)
}
