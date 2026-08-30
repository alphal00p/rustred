//! Generated positive-dot rays on two factorized three-line geometries.
//!
//! One ray has the dot on the middle edge of the three-line path; the other
//! has it on a spoke of the three-line star.  In both cases an inactive edge
//! carries one numerator power.  The complete depth-zero ordinary K6 span
//! selects two rows, which production independently reprojects on the full
//! positive source ray.  Each recurrence lowers the dot directly into the
//! corresponding authenticated factorization domain.

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::identity::{
    CompletedIbpSourceRows, ParametricIbpConfig, ParametricIbpGenerator, TranslatedSourceLimits,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::support::complete_ordinary_sources;

pub(super) const PATH_SOURCE_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
pub(super) const STAR_SOURCE_SECTOR: [i64; 6] = [0, 0, 1, 1, 0, 1];
pub(super) const PATH_MIDDLE_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 0, -1, 0, 1];
pub(super) const STAR_SPOKE_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 0, 0, -1, 1];
pub(super) const FREE_POSITION: usize = 5;
const SEARCH_DEPTH: usize = 0;

pub(super) struct FactorizedThreeLineDotSelectionWitness {
    pub(super) complete_path_sources: SourceViewBatch,
    pub(super) complete_path_rule: ParametricRule,
    pub(super) complete_star_sources: SourceViewBatch,
    pub(super) complete_star_rule: ParametricRule,
}

pub(super) struct FactorizedThreeLineDotNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) path_middle_ray: RuleCell,
    pub(super) star_spoke_ray: RuleCell,
    pub(super) path_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) star_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<FactorizedThreeLineDotSelectionWitness>,
}

pub(super) fn derive_factorized_three_line_dot_numerator_rays()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_factorized_three_line_dot_numerator_build(false)?;
    Ok((build.context, build.path_middle_ray, build.star_spoke_ray))
}

pub(super) fn derive_factorized_three_line_dot_numerator_build(
    retain_selection_witness: bool,
) -> Result<FactorizedThreeLineDotNumeratorBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;

    let path = derive_ray(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        PATH_SOURCE_SECTOR,
        &PATH_MIDDLE_DOT_NUMERATOR_PIVOT,
    )?;
    let star = derive_ray(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        STAR_SOURCE_SECTOR,
        &STAR_SPOKE_DOT_NUMERATOR_PIVOT,
    )?;

    let selection_witness =
        retain_selection_witness.then_some(FactorizedThreeLineDotSelectionWitness {
            complete_path_sources: path.complete_sources,
            complete_path_rule: path.complete_rule,
            complete_star_sources: star.complete_sources,
            complete_star_rule: star.complete_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(FactorizedThreeLineDotNumeratorBuild {
        context,
        path_middle_ray: path.cell,
        star_spoke_ray: star.cell,
        path_selected_complete_source_ordinals: path.selected_complete_source_ordinals,
        star_selected_complete_source_ordinals: star.selected_complete_source_ordinals,
        selection_witness,
    })
}

struct GeneratedRay {
    complete_sources: SourceViewBatch,
    complete_rule: ParametricRule,
    selected_complete_source_ordinals: Box<[usize]>,
    cell: RuleCell,
}

fn derive_ray(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    sector: [i64; 6],
    pivot: &[i64; 6],
) -> Result<GeneratedRay, ArtifactError> {
    let search = depth_search(sector)?;
    let complete_sources = project_complete_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        source_domain(sector)?,
        fixed_source(sector),
        canonicalizer,
        zero_sectors,
    )?;
    let complete_rule = derive_rule(generator, &complete_sources, &sector, pivot)?;
    let selected_complete_source_ordinals = selected_source_ordinals(&complete_rule);
    let selected_sources = project_selected_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        &selected_complete_source_ordinals,
        source_domain(sector)?,
        fixed_source(sector),
        canonicalizer,
        zero_sectors,
    )?;
    let selected_rule = derive_rule(generator, &selected_sources, &sector, pivot)?;
    let application = application_domain(&selected_rule, sector)?;
    let cell = RuleCell::try_refined(
        generator.context(),
        selected_rule,
        selected_sources,
        application,
        fixed_source(sector),
        [],
        RuleCellLimits::default(),
    )?;
    Ok(GeneratedRay {
        complete_sources,
        complete_rule,
        selected_complete_source_ordinals,
        cell,
    })
}

fn derive_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
    sector: &[i64; 6],
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        sector,
        pivot,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_complete_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    domain: SectorInteriorDomain,
    fixed: impl IntoIterator<Item = FixedIndexRestriction>,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        translations,
        TranslatedSourceLimits::default(),
    )?;
    Ok(SourceViewBatch::try_project_complete_residual(
        translated,
        generator.context(),
        domain,
        fixed,
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn project_selected_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: &[usize],
    domain: SectorInteriorDomain,
    fixed: impl IntoIterator<Item = FixedIndexRestriction>,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        translations,
        TranslatedSourceLimits::default(),
    )?;
    Ok(SourceViewBatch::try_project_residual(
        translated,
        ordinals,
        generator.context(),
        domain,
        fixed,
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn depth_search(sector: [i64; 6]) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(sector)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn search_depth() -> usize {
    SEARCH_DEPTH
}

#[cfg(test)]
pub(super) fn derive_complete_path_candidate() -> Result<ParametricRule, ArtifactError> {
    derive_complete_candidate(PATH_SOURCE_SECTOR, &PATH_MIDDLE_DOT_NUMERATOR_PIVOT)
}

#[cfg(test)]
pub(super) fn derive_complete_star_candidate() -> Result<ParametricRule, ArtifactError> {
    derive_complete_candidate(STAR_SOURCE_SECTOR, &STAR_SPOKE_DOT_NUMERATOR_PIVOT)
}

#[cfg(test)]
fn derive_complete_candidate(
    sector: [i64; 6],
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search(sector)?;
    let sources = project_complete_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        source_domain(sector)?,
        fixed_source(sector),
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_rule(&generator, &sources, &sector, pivot)
}

pub(super) fn fixed_source(sector: [i64; 6]) -> [FixedIndexRestriction; 5] {
    std::array::from_fn(|slot| {
        let position = if slot < FREE_POSITION { slot } else { slot + 1 };
        FixedIndexRestriction::new(position, sector[position])
    })
}

fn source_domain(sector: [i64; 6]) -> Result<SectorInteriorDomain, ArtifactError> {
    let mut bounds = sector.map(|power| InteriorBounds::new(power, power));
    bounds[FREE_POSITION] = InteriorBounds::new(1, i64::MAX);
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&sector)?,
        bounds,
    )?)
}

fn application_domain(
    rule: &ParametricRule,
    sector_powers: [i64; 6],
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let sector = Mask::try_from_indices(&sector_powers)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = sector_powers.map(|power| InteriorBounds::new(power, power));
    bounds[FREE_POSITION] = InteriorBounds::new(1, maximal.bounds()[FREE_POSITION].upper());
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}

fn selected_source_ordinals(rule: &ParametricRule) -> Box<[usize]> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice()
}
