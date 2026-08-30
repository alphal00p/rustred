//! Exact endpoint with an inactive numerator incident to both active dots.
//!
//! This module owns one `S4` orbit of `J(0,1,2,2,1,-1)`.  A complete
//! depth-one search selects five of the 63 translated ordinary K6 rows.
//! Production independently retranslates and reprojects only those rows on
//! the exact scalar four-line corner.  The cell deliberately does not claim
//! deeper numerators, higher active powers, or inequivalent two-dot
//! placements.

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

use super::super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::super::support::complete_ordinary_sources;
use super::super::FOUR_LINE_SECTOR;

pub(super) const INCIDENT_TWO_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 1, 1, 0, -1];
const SEARCH_DEPTH: usize = 1;

pub(super) struct IncidentTwoDotSelectionWitness {
    pub(super) complete_sources: SourceViewBatch,
    pub(super) complete_rule: ParametricRule,
}

pub(super) struct IncidentTwoDotEndpointBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<IncidentTwoDotSelectionWitness>,
}

pub(in super::super) fn derive_incident_two_dot_numerator_endpoint()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_incident_two_dot_endpoint_build(false)?;
    Ok((build.context, build.endpoint))
}

pub(super) fn derive_incident_two_dot_endpoint_build(
    retain_selection_witness: bool,
) -> Result<IncidentTwoDotEndpointBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search(SEARCH_DEPTH)?;

    let complete_sources = project_complete_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_rule = derive_endpoint_rule(&generator, &complete_sources)?;
    let selected_complete_source_ordinals = complete_rule
        .source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let selected_sources = project_selected_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let selected_rule = derive_endpoint_rule(&generator, &selected_sources)?;
    let application = endpoint_application_domain(&selected_rule)?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        selected_rule,
        selected_sources,
        application,
        fixed_endpoint(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness = retain_selection_witness.then_some(IncidentTwoDotSelectionWitness {
        complete_sources,
        complete_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(IncidentTwoDotEndpointBuild {
        context,
        endpoint,
        selected_complete_source_ordinals,
        selection_witness,
    })
}

fn derive_endpoint_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &FOUR_LINE_SECTOR,
        &INCIDENT_TWO_DOT_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_complete_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
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
        endpoint_source_domain()?,
        fixed_endpoint(),
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
        endpoint_source_domain()?,
        fixed_endpoint(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn depth_search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn incident_two_dot_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_endpoint() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, FOUR_LINE_SECTOR[position]))
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power)),
    )?)
}

fn endpoint_application_domain(
    rule: &ParametricRule,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power)),
        rule.pivot().values(),
        &rhs,
    )?)
}

#[cfg(test)]
pub(super) fn derive_incident_two_dot_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search(depth)?;
    let sources = project_complete_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_endpoint_rule(&generator, &sources)
}
