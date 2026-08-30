//! Shared exact-corner projection and singleton-cell construction.

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

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::support::complete_ordinary_sources;
use super::FOUR_LINE_SECTOR;

const BASE_CORNER: [i64; 6] = FOUR_LINE_SECTOR;

pub(super) fn derive_same_sector_candidate(
    target_shift: &[i64; 6],
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
        target_shift,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

pub(super) fn fixed_base_corner() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, BASE_CORNER[position]))
}

pub(super) fn derive_exact_corner_cell(
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

pub(super) fn project_exact_corner_sources(
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

pub(super) fn project_complete_exact_corner_sources(
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
