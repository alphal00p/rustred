//! Positive-dot recurrences on the canonical five-line residual face.

use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::FIVE_LINE_SECTOR;

pub(super) const ANCHOR: [i64; 6] = [0, 2, 2, 2, 2, 2];
pub(super) const ADJACENT_EDGE_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 1, 0];
pub(super) const OPPOSITE_EDGE_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 0, 1];

pub(super) struct PositiveDotCells {
    pub(super) adjacent: RuleCell,
    pub(super) opposite: RuleCell,
}

pub(super) fn derive_positive_dot_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<PositiveDotCells, ArtifactError> {
    let adjacent_sources = projected_sources(
        generator,
        completed,
        source_count,
        canonicalizer,
        zero_sectors,
    )?;
    let adjacent_rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        adjacent_sources.relations(),
        &ANCHOR,
        &ADJACENT_EDGE_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let adjacent_application = five_line_application_domain(&adjacent_rule)?;
    let adjacent = RuleCell::try_refined(
        generator.context(),
        adjacent_rule,
        adjacent_sources,
        adjacent_application,
        [FixedIndexRestriction::new(0, 0)],
        [],
        RuleCellLimits::default(),
    )?;

    let opposite_sources = projected_sources(
        generator,
        completed,
        source_count,
        canonicalizer,
        zero_sectors,
    )?;
    let opposite_rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        opposite_sources.relations(),
        &ANCHOR,
        &OPPOSITE_EDGE_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let opposite_application = five_line_application_domain(&opposite_rule)?;
    let opposite = RuleCell::try_refined(
        generator.context(),
        opposite_rule,
        opposite_sources,
        opposite_application,
        [FixedIndexRestriction::new(0, 0)],
        [],
        RuleCellLimits::default(),
    )?;
    Ok(PositiveDotCells { adjacent, opposite })
}

fn projected_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        [IntegralShift::try_new([0; 6])?],
        TranslatedSourceLimits::default(),
    )?;
    let domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FIVE_LINE_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
        ],
    )?;
    let ordinals = (0..source_count).collect::<Vec<_>>();
    Ok(SourceViewBatch::try_project_residual(
        translated,
        &ordinals,
        generator.context(),
        domain,
        [FixedIndexRestriction::new(0, 0)],
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn five_line_application_domain(
    rule: &ParametricRule,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let sector = Mask::try_from_indices(&FIVE_LINE_SECTOR)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = maximal.bounds().to_vec();
    bounds[0] = InteriorBounds::new(0, 0);
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}
