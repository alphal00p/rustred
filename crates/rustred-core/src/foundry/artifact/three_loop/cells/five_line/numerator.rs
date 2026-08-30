//! Negative inactive-power recurrences on the five-line face.
//!
//! One generated pivot raises the inactive power toward zero.  Disjoint bulk
//! and endpoint cells retain the same generated rule; endpoint branches are
//! removed only through exact `n0 = 0` coefficient specialization.

use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_interior_rule_for_target,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceLimits,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::FIVE_LINE_SECTOR;

pub(super) const NUMERATOR_PIVOT: [i64; 6] = [-1, 0, 0, 0, 0, 0];
pub(super) const NUMERATOR_RULE_ANCHOR: [i64; 6] = [-2, 3, 3, 3, 3, 3];
pub(super) const ADJACENT_DOT_POSITION: usize = 4;
pub(super) const OPPOSITE_DOT_POSITION: usize = 5;

pub(super) const SCALAR_TRANSLATIONS: [[i64; 6]; 3] =
    [[-1, 0, 0, 0, 0, 0], [0, 0, 0, 0, 0, -1], [0, 0, 0, 0, 0, 0]];
pub(super) const SCALAR_SELECTION: [(usize, usize); 5] = [(0, 0), (0, 3), (1, 0), (2, 0), (2, 3)];
pub(super) const SCALAR_ENDPOINT_PRUNED: [usize; 2] = [4, 7];

pub(super) const ADJACENT_TRANSLATIONS: [[i64; 6]; 1] = [[0, 0, 0, 0, -1, 0]];
pub(super) const ADJACENT_SELECTION: [(usize, usize); 1] = [(0, 4)];
pub(super) const ADJACENT_ENDPOINT_PRUNED: [usize; 0] = [];

pub(super) const OPPOSITE_TRANSLATIONS: [[i64; 6]; 3] = SCALAR_TRANSLATIONS;
pub(super) const OPPOSITE_SELECTION: [(usize, usize); 12] = [
    (0, 3),
    (0, 4),
    (0, 5),
    (1, 2),
    (1, 8),
    (2, 0),
    (2, 1),
    (2, 2),
    (2, 3),
    (2, 4),
    (2, 7),
    (2, 8),
];
pub(super) const OPPOSITE_ENDPOINT_PRUNED: [usize; 3] = [13, 19, 20];

pub(super) struct NegativeNumeratorCells {
    pub(super) scalar_endpoint: RuleCell,
    pub(super) scalar_bulk: RuleCell,
    pub(super) adjacent_endpoint: RuleCell,
    pub(super) adjacent_bulk: RuleCell,
    pub(super) opposite_endpoint: RuleCell,
    pub(super) opposite_bulk: RuleCell,
}

pub(super) fn derive_negative_numerator_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<NegativeNumeratorCells, ArtifactError> {
    let (scalar_endpoint, scalar_bulk) = build_scalar_cells(
        generator,
        completed,
        source_count,
        canonicalizer,
        zero_sectors,
    )?;
    let (adjacent_endpoint, adjacent_bulk) = build_active_cells(
        generator,
        completed,
        source_count,
        &ADJACENT_TRANSLATIONS,
        &ADJACENT_SELECTION,
        ADJACENT_DOT_POSITION,
        &ADJACENT_ENDPOINT_PRUNED,
    )?;
    let (opposite_endpoint, opposite_bulk) = build_active_cells(
        generator,
        completed,
        source_count,
        &OPPOSITE_TRANSLATIONS,
        &OPPOSITE_SELECTION,
        OPPOSITE_DOT_POSITION,
        &OPPOSITE_ENDPOINT_PRUNED,
    )?;
    Ok(NegativeNumeratorCells {
        scalar_endpoint,
        scalar_bulk,
        adjacent_endpoint,
        adjacent_bulk,
        opposite_endpoint,
        opposite_bulk,
    })
}

fn build_scalar_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<(RuleCell, RuleCell), ArtifactError> {
    let sources = scalar_corner_sources(
        generator,
        completed,
        source_count,
        &SCALAR_SELECTION,
        canonicalizer,
        zero_sectors,
    )?;
    let rule = derive_numerator_rule(generator, &sources)?;
    let bulk_domain = scalar_application_domain(&rule, InteriorBounds::new(i64::MIN + 1, -1))?;
    let fixed_active = (1..6)
        .map(|position| FixedIndexRestriction::new(position, 1))
        .collect::<Vec<_>>();
    let bulk = RuleCell::try_refined(
        generator.context(),
        rule.clone(),
        sources,
        bulk_domain,
        fixed_active,
        [],
        RuleCellLimits::default(),
    )?;

    let endpoint_sources = scalar_corner_sources(
        generator,
        completed,
        source_count,
        &SCALAR_SELECTION,
        canonicalizer,
        zero_sectors,
    )?;
    let endpoint_domain = scalar_application_domain(&rule, InteriorBounds::new(0, 0))?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        rule,
        endpoint_sources,
        endpoint_domain,
        (0..6).map(|position| {
            FixedIndexRestriction::new(position, if position == 0 { 0 } else { 1 })
        }),
        SCALAR_ENDPOINT_PRUNED,
        RuleCellLimits::default(),
    )?;
    Ok((endpoint, bulk))
}

#[allow(clippy::too_many_arguments)]
fn build_active_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    translations: &[[i64; 6]],
    selection: &[(usize, usize)],
    dotted_position: usize,
    endpoint_pruned: &[usize],
) -> Result<(RuleCell, RuleCell), ArtifactError> {
    let sources =
        direct_selected_sources(generator, completed, source_count, translations, selection)?;
    let rule = derive_numerator_rule(generator, &sources)?;
    let bulk_domain = active_application_domain(
        &rule,
        InteriorBounds::new(i64::MIN + 1, -1),
        dotted_position,
    )?;
    let bulk = RuleCell::try_refined(
        generator.context(),
        rule.clone(),
        sources,
        bulk_domain,
        [],
        [],
        RuleCellLimits::default(),
    )?;

    let endpoint_sources =
        direct_selected_sources(generator, completed, source_count, translations, selection)?;
    let endpoint_domain =
        active_application_domain(&rule, InteriorBounds::new(0, 0), dotted_position)?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        rule,
        endpoint_sources,
        endpoint_domain,
        [FixedIndexRestriction::new(0, 0)],
        endpoint_pruned.iter().copied(),
        RuleCellLimits::default(),
    )?;
    Ok((endpoint, bulk))
}

pub(super) fn derive_numerator_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &NUMERATOR_RULE_ANCHOR,
        &NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

pub(super) fn scalar_corner_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    selection: &[(usize, usize)],
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        SCALAR_TRANSLATIONS
            .into_iter()
            .map(IntegralShift::try_new)
            .collect::<Result<Vec<_>, _>>()?,
        TranslatedSourceLimits::default(),
    )?;
    let ordinals = flattened_ordinals(source_count, SCALAR_TRANSLATIONS.len(), selection)?;
    let domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FIVE_LINE_SECTOR)?,
        [
            InteriorBounds::new(i64::MIN + 1, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
    )?;
    Ok(SourceViewBatch::try_project_residual(
        translated,
        &ordinals,
        generator.context(),
        domain,
        (1..6).map(|position| FixedIndexRestriction::new(position, 1)),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

pub(super) fn direct_selected_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    source_count: usize,
    translations: &[[i64; 6]],
    selection: &[(usize, usize)],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        translations
            .iter()
            .copied()
            .map(IntegralShift::try_new)
            .collect::<Result<Vec<_>, _>>()?,
        TranslatedSourceLimits::default(),
    )?;
    let ordinals = flattened_ordinals(source_count, translations.len(), selection)?;
    Ok(SourceViewBatch::try_select(
        translated,
        &ordinals,
        RuleCellLimits::default(),
    )?)
}

pub(super) fn flattened_ordinals(
    source_count: usize,
    translation_count: usize,
    selection: &[(usize, usize)],
) -> Result<Vec<usize>, ArtifactError> {
    let mut ordinals = Vec::with_capacity(selection.len());
    for &(translation, row) in selection {
        if translation >= translation_count || row >= source_count {
            return Err(ArtifactError::InvalidReplayEvidence {
                detail: "five-line numerator source selection is outside the generated span",
            });
        }
        let ordinal = translation
            .checked_mul(source_count)
            .and_then(|base| base.checked_add(row))
            .ok_or(ArtifactError::InvalidReplayEvidence {
                detail: "five-line numerator source ordinal overflowed",
            })?;
        ordinals.push(ordinal);
    }
    Ok(ordinals)
}

fn scalar_application_domain(
    rule: &ParametricRule,
    inactive: InteriorBounds,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rhs_shifts(rule);
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FIVE_LINE_SECTOR)?,
        [
            inactive,
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        rule.pivot().values(),
        &rhs,
    )?)
}

fn active_application_domain(
    rule: &ParametricRule,
    inactive: InteriorBounds,
    dotted_position: usize,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rhs_shifts(rule);
    let sector = Mask::try_from_indices(&FIVE_LINE_SECTOR)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = maximal.bounds().to_vec();
    bounds[0] = inactive;
    bounds[dotted_position] = InteriorBounds::new(2, bounds[dotted_position].upper());
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}

fn rhs_shifts(rule: &ParametricRule) -> Vec<&[i64]> {
    rule.right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect()
}
