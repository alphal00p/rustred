use std::collections::BTreeSet;

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
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

use super::super::manifest::ZERO_ORBITS;
use super::super::{canonical_family, canonical_s4};

const FIVE_LINE_SECTOR: [i64; 6] = [0, 1, 1, 1, 1, 1];
const ANCHOR: [i64; 6] = [0, 2, 2, 2, 2, 2];
const ADJACENT_EDGE_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 1, 0];
const OPPOSITE_EDGE_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 0, 1];

/// Derive exact projected recurrences for the two inequivalent dotted-edge
/// orbits on the canonical five-line residual face. These test-only cells are
/// discovery slices, not a claim that the five-line sector (or the `K = 6`
/// artifact) is closed.
fn derive_five_line_cells() -> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError>
{
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, source_count) = complete_ordinary_sources(&generator)?;
    let adjacent_sources = projected_sources(
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
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
        &generator,
        &completed,
        source_count,
        &canonicalizer,
        &zero_sectors,
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
    let context = generator.context().clone();
    let opposite = RuleCell::try_refined(
        generator.context(),
        opposite_rule,
        opposite_sources,
        opposite_application,
        [FixedIndexRestriction::new(0, 0)],
        [],
        RuleCellLimits::default(),
    )?;
    drop(generator);
    Ok((context, adjacent, opposite))
}

fn complete_ordinary_sources(
    generator: &ParametricIbpGenerator<'_>,
) -> Result<(CompletedIbpSourceRows, usize), ArtifactError> {
    let prepared = generator.prepare_ordinary_ibp()?;
    let source_count = prepared.len();
    let rows = (0..source_count)
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    Ok((prepared.complete(rows)?, source_count))
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

fn exact_zero_sectors(
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
) -> Result<Vec<Mask>, ArtifactError> {
    let zero_representatives = ZERO_ORBITS
        .iter()
        .map(|orbit| orbit.representative)
        .collect::<BTreeSet<_>>();
    (0_u64..64)
        .map(|bits| {
            let powers: [i64; 6] = std::array::from_fn(|slot| i64::from(((bits >> slot) & 1) != 0));
            let key = IntegralKey::try_new(powers)?;
            let canonical = canonicalizer.canonicalize(&key)?;
            Ok::<_, ArtifactError>(
                zero_representatives
                    .contains(canonical.canonical().powers())
                    .then(|| Mask::try_from_indices(key.powers()))
                    .transpose()?,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|sectors| sectors.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests;
