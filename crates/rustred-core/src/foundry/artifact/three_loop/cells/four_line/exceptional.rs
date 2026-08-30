use crate::algebra::IndexedCoefficientContext;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
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
const OPPOSITE_PAIR_SOURCE_ORDINALS: [usize; 4] = [0, 1, 3, 8];
const OPPOSITE_PAIR_SOURCE_SHIFT: [i64; 6] = [0, 0, 1, 0, 0, 0];
pub(super) const ADJACENT_DOT_PAIR_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 1, 1, 0];
pub(super) const OPPOSITE_DOT_PAIR_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 0, 1, 0];

/// Derive the two exact singleton cells currently owned on the scalar
/// four-line corner.
///
/// The first lowers the isolated canonical dot excluded by the ordinary
/// positive-box recurrence. The second lowers the opposite two-dot orbit from
/// one translated ordinary-source layer. Both specialize the whole base
/// corner so its order-eight setwise stabilizer can route equivalent edge
/// decorations. The adjacent two-dot orbit and every numerator face remain
/// explicit closure obligations.
pub(super) fn derive_exceptional_four_line_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
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

    let opposite_sources = project_exact_corner_sources(
        &generator,
        &completed,
        &OPPOSITE_PAIR_SOURCE_ORDINALS,
        &canonicalizer,
        &zero_sectors,
        OPPOSITE_PAIR_SOURCE_SHIFT,
    )?;
    let opposite = derive_exact_corner_cell(
        &generator,
        opposite_sources,
        &OPPOSITE_DOT_PAIR_TARGET_SHIFT,
    )?;

    let context = generator.context().clone();
    drop(generator);
    Ok((context, isolated, opposite))
}

/// Re-run the bounded full-span adjacent-pair candidate used to retain a
/// concise typed negative witness. This does not authenticate an exhaustive
/// translated-source search or turn the rejected orbit into a terminal.
pub(super) fn derive_adjacent_full_span_candidate(
    translation: [i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, source_count) = complete_ordinary_sources(&generator)?;
    let ordinals = (0..source_count).collect::<Vec<_>>();
    let sources = project_exact_corner_sources(
        &generator,
        &completed,
        &ordinals,
        &canonicalizer,
        &zero_sectors,
        translation,
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

fn singleton_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        BASE_CORNER.map(|value| InteriorBounds::new(value, value)),
    )?)
}
