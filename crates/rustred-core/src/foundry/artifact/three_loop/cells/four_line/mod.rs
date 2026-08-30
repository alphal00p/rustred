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

use super::super::exact_zero_sectors;
use super::super::{canonical_family, canonical_s4};
use super::support::complete_ordinary_sources;

mod complementary_mixed_dot;
#[cfg(test)]
mod complementary_mixed_dot_tests;
mod corner;
mod exceptional;
#[cfg(test)]
mod exceptional_tests;
mod mixed_dot;
mod mixed_dot_ray;
#[cfg(test)]
mod mixed_dot_ray_tests;
#[cfg(test)]
mod mixed_dot_tests;
mod repeated_dot;
#[cfg(test)]
mod repeated_dot_tests;

use complementary_mixed_dot::derive_complementary_mixed_dot_cell;
use exceptional::{ExceptionalFourLineCells, derive_exceptional_four_line_cells};
use mixed_dot::{MixedDotFourLineCells, derive_mixed_dot_four_line_cells};
use mixed_dot_ray::derive_mixed_dot_ray_cell;
use repeated_dot::derive_repeated_dot_ray_cell;

const FOUR_LINE_SECTOR: [i64; 6] = [0, 1, 1, 1, 1, 0];
const ANCHOR: [i64; 6] = [0, 2, 2, 2, 2, 0];
const CANONICAL_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 1, 0];
const MIXED_NUMERATOR_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 1, -1];
const CANONICAL_TARGET_SOURCE_SHIFT: [i64; 6] = [0, -1, 0, 0, 1, 0];
const ZERO_SOURCE_SHIFT: [i64; 6] = [0; 6];

/// Complete ordered owner of the currently derived four-line discovery
/// slices.  The singleton corner exceptions and selected-source ray precede
/// the broad positive-box cells so future domain refinements cannot silently
/// change first-applicable ownership on their certified domains.
pub(super) struct FourLineCellSet {
    pub(super) isolated: RuleCell,
    pub(super) opposite: RuleCell,
    pub(super) adjacent: RuleCell,
    pub(super) triple: RuleCell,
    pub(super) three_distinct: RuleCell,
    pub(super) adjacent_mixed_dot: RuleCell,
    pub(super) opposite_mixed_dot: RuleCell,
    pub(super) complementary_mixed_dot: RuleCell,
    pub(super) mixed_dot_ray: RuleCell,
    pub(super) repeated_dot_ray: RuleCell,
    pub(super) canonical_dot: RuleCell,
    pub(super) mixed_numerator: RuleCell,
}

pub(super) fn derive_all_four_line_cells() -> Result<FourLineCellSet, ArtifactError> {
    let ExceptionalFourLineCells {
        context: _,
        isolated,
        opposite,
        adjacent,
        triple,
        three_distinct,
    } = derive_exceptional_four_line_cells()?;
    let MixedDotFourLineCells {
        context: _,
        adjacent: adjacent_mixed_dot,
        opposite: opposite_mixed_dot,
    } = derive_mixed_dot_four_line_cells()?;
    let complementary_mixed_dot = derive_complementary_mixed_dot_cell()?.cell;
    let (_context, mixed_dot_ray) = derive_mixed_dot_ray_cell()?;
    let (_context, repeated_dot_ray) = derive_repeated_dot_ray_cell()?;
    let (_context, canonical_dot, mixed_numerator) = derive_four_line_cells()?;
    Ok(FourLineCellSet {
        isolated,
        opposite,
        adjacent,
        triple,
        three_distinct,
        adjacent_mixed_dot,
        opposite_mixed_dot,
        complementary_mixed_dot,
        mixed_dot_ray,
        repeated_dot_ray,
        canonical_dot,
        mixed_numerator,
    })
}

/// Derive exact projected recurrences for one canonical active-line dot and
/// one mixed numerator/dot boundary on the canonical four-line residual
/// face. These test-only cells are discovery slices, not a claim that the
/// four-line sector or `K = 6` artifact is closed.
fn derive_four_line_cells() -> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError>
{
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, source_count) = complete_ordinary_sources(&generator)?;
    let canonical_dot_sources = projected_sources(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
        CANONICAL_TARGET_SOURCE_SHIFT,
    )?;
    let canonical_dot_rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        canonical_dot_sources.relations(),
        &ANCHOR,
        &CANONICAL_DOT_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    // The exact translated-source guards include n1 - 1, so n1 >= 2 is the
    // maximal positive half-box on which every guard is uniformly nonzero.
    let canonical_dot_application = four_line_application_domain(&canonical_dot_rule, 2)?;
    let canonical_dot = RuleCell::try_refined(
        generator.context(),
        canonical_dot_rule,
        canonical_dot_sources,
        canonical_dot_application,
        fixed_inactive_indices(),
        [],
        RuleCellLimits::default(),
    )?;

    let mixed_sources = projected_sources(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
        ZERO_SOURCE_SHIFT,
    )?;
    let mixed_rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        mixed_sources.relations(),
        &ANCHOR,
        &MIXED_NUMERATOR_DOT_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let mixed_application = four_line_application_domain(&mixed_rule, 1)?;
    let context = generator.context().clone();
    let mixed = RuleCell::try_refined(
        generator.context(),
        mixed_rule,
        mixed_sources,
        mixed_application,
        fixed_inactive_indices(),
        [],
        RuleCellLimits::default(),
    )?;
    drop(generator);
    Ok((context, canonical_dot, mixed))
}

fn fixed_inactive_indices() -> [FixedIndexRestriction; 2] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(5, 0),
    ]
}

fn projected_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    translation: [i64; 6],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        [IntegralShift::try_new(translation)?],
        TranslatedSourceLimits::default(),
    )?;
    let domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(0, 0),
        ],
    )?;
    let ordinals = (0..source_count).collect::<Vec<_>>();
    Ok(SourceViewBatch::try_project_residual(
        translated,
        &ordinals,
        generator.context(),
        domain,
        fixed_inactive_indices(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn four_line_application_domain(
    rule: &ParametricRule,
    guard_safe_first_active_lower: i64,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let sector = Mask::try_from_indices(&FOUR_LINE_SECTOR)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = maximal.bounds().to_vec();
    bounds[0] = InteriorBounds::new(0, 0);
    bounds[1] = InteriorBounds::new(guard_safe_first_active_lower, bounds[1].upper());
    bounds[5] = InteriorBounds::new(0, 0);
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}

#[cfg(test)]
mod tests;
