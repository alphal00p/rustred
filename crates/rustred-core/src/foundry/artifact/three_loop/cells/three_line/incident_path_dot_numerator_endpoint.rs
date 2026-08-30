//! Exact incident dot/numerator endpoint on the three-line path.
//!
//! The source `J(0,0,1,-1,1,1)` is already owned by the undotted path
//! numerator lane.  A complete untranslated span of the nine ordinary K6
//! rows selects two relations for `J(0,0,1,-1,2,1)`; production independently
//! reprojects precisely those rows.  The singleton cell claims no other
//! placement orbit, deeper numerator, or higher active power.

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_interior_rule_for_target,
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
use super::undotted_path_numerator::UNDOTTED_PATH_SECTOR;

pub(super) const INCIDENT_PATH_SOURCE: [i64; 6] = [0, 0, 1, -1, 1, 1];
pub(super) const INCIDENT_PATH_REPLAY_ANCHOR: [i64; 6] = [-2, -2, 3, -2, 3, 3];
pub(super) const INCIDENT_PATH_DOT_PIVOT: [i64; 6] = [0, 0, 0, 0, 1, 0];
const SEARCH_DEPTH: usize = 0;

pub(super) struct IncidentPathSelectionWitness {
    pub(super) complete_sources: SourceViewBatch,
    pub(super) complete_rule: ParametricRule,
}

pub(super) struct IncidentPathEndpointBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<IncidentPathSelectionWitness>,
}

pub(super) fn derive_incident_path_dot_numerator_endpoint()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_incident_path_endpoint_build(false)?;
    Ok((build.context, build.endpoint))
}

pub(super) fn derive_incident_path_endpoint_build(
    retain_selection_witness: bool,
) -> Result<IncidentPathEndpointBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search()?;

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
        fixed_source(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness = retain_selection_witness.then_some(IncidentPathSelectionWitness {
        complete_sources,
        complete_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(IncidentPathEndpointBuild {
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
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &INCIDENT_PATH_REPLAY_ANCHOR,
        &INCIDENT_PATH_DOT_PIVOT,
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
        fixed_source(),
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
        fixed_source(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn depth_search() -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(UNDOTTED_PATH_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn incident_path_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_source() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, INCIDENT_PATH_SOURCE[position])
    })
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&UNDOTTED_PATH_SECTOR)?,
        INCIDENT_PATH_SOURCE.map(|power| InteriorBounds::new(power, power)),
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
        Mask::try_from_indices(&UNDOTTED_PATH_SECTOR)?,
        INCIDENT_PATH_SOURCE.map(|power| InteriorBounds::new(power, power)),
        rule.pivot().values(),
        &rhs,
    )?)
}
