//! Exact endpoints with numerators on a pair of opposite inactive edges.
//!
//! Both cells are derived at the already-certified scalar-numerator source
//! `J(0,1,1,1,1,-1)`.  The undotted target requires a complete depth-one
//! search; its one-dot child is available at depth zero.  In each case the
//! complete ordinary K6 span determines the selected rows and production
//! independently retranslates and reprojects only that selection.

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

use super::super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::super::support::complete_ordinary_sources;
use super::super::FOUR_LINE_SECTOR;

pub(super) const OPPOSITE_PAIR_SOURCE: [i64; 6] = [0, 1, 1, 1, 1, -1];
pub(super) const OPPOSITE_PAIR_REPLAY_ANCHOR: [i64; 6] = [-2, 3, 3, 3, 3, -2];
pub(super) const OPPOSITE_PAIR_PIVOT: [i64; 6] = [-1, 0, 0, 0, 0, 0];
pub(super) const OPPOSITE_PAIR_DOT_PIVOT: [i64; 6] = [-1, 0, 0, 0, 1, 0];
const UNDOTTED_SEARCH_DEPTH: usize = 1;
const DOTTED_SEARCH_DEPTH: usize = 0;

pub(super) struct OppositePairSelectionWitness {
    pub(super) complete_undotted_sources: SourceViewBatch,
    pub(super) complete_undotted_rule: ParametricRule,
    pub(super) complete_dotted_sources: SourceViewBatch,
    pub(super) complete_dotted_rule: ParametricRule,
}

pub(super) struct OppositePairEndpointBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) undotted_endpoint: RuleCell,
    pub(super) dotted_endpoint: RuleCell,
    pub(super) undotted_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) dotted_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<OppositePairSelectionWitness>,
}

pub(in super::super) fn derive_opposite_inactive_numerator_pair_endpoints()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_opposite_pair_endpoint_build(false)?;
    Ok((
        build.context,
        build.undotted_endpoint,
        build.dotted_endpoint,
    ))
}

pub(super) fn derive_opposite_pair_endpoint_build(
    retain_selection_witness: bool,
) -> Result<OppositePairEndpointBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;

    let undotted_search = depth_search(UNDOTTED_SEARCH_DEPTH)?;
    let complete_undotted_sources = project_complete_sources(
        &generator,
        &completed,
        undotted_search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_undotted_rule =
        derive_endpoint_rule(&generator, &complete_undotted_sources, &OPPOSITE_PAIR_PIVOT)?;
    let undotted_selected_complete_source_ordinals =
        selected_source_ordinals(&complete_undotted_rule);
    let undotted_sources = project_selected_sources(
        &generator,
        &completed,
        undotted_search.offsets().iter().cloned(),
        &undotted_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let undotted_rule = derive_endpoint_rule(&generator, &undotted_sources, &OPPOSITE_PAIR_PIVOT)?;
    let undotted_endpoint = build_endpoint(&generator, undotted_rule, undotted_sources)?;

    let dotted_search = depth_search(DOTTED_SEARCH_DEPTH)?;
    let complete_dotted_sources = project_complete_sources(
        &generator,
        &completed,
        dotted_search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_dotted_rule = derive_endpoint_rule(
        &generator,
        &complete_dotted_sources,
        &OPPOSITE_PAIR_DOT_PIVOT,
    )?;
    let dotted_selected_complete_source_ordinals = selected_source_ordinals(&complete_dotted_rule);
    let dotted_sources = project_selected_sources(
        &generator,
        &completed,
        dotted_search.offsets().iter().cloned(),
        &dotted_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let dotted_rule = derive_endpoint_rule(&generator, &dotted_sources, &OPPOSITE_PAIR_DOT_PIVOT)?;
    let dotted_endpoint = build_endpoint(&generator, dotted_rule, dotted_sources)?;

    let selection_witness = retain_selection_witness.then_some(OppositePairSelectionWitness {
        complete_undotted_sources,
        complete_undotted_rule,
        complete_dotted_sources,
        complete_dotted_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(OppositePairEndpointBuild {
        context,
        undotted_endpoint,
        dotted_endpoint,
        undotted_selected_complete_source_ordinals,
        dotted_selected_complete_source_ordinals,
        selection_witness,
    })
}

fn derive_endpoint_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &OPPOSITE_PAIR_REPLAY_ANCHOR,
        pivot,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn build_endpoint(
    generator: &ParametricIbpGenerator<'_>,
    rule: ParametricRule,
    sources: SourceViewBatch,
) -> Result<RuleCell, ArtifactError> {
    let application = endpoint_application_domain(&rule)?;
    Ok(RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed_source(),
        [],
        RuleCellLimits::default(),
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

fn depth_search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn undotted_search_depth() -> usize {
    UNDOTTED_SEARCH_DEPTH
}

pub(super) const fn dotted_search_depth() -> usize {
    DOTTED_SEARCH_DEPTH
}

#[cfg(test)]
pub(super) fn derive_opposite_pair_candidate(
    depth: usize,
    pivot: &[i64; 6],
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
    derive_endpoint_rule(&generator, &sources, pivot)
}

pub(super) fn fixed_source() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, OPPOSITE_PAIR_SOURCE[position])
    })
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        OPPOSITE_PAIR_SOURCE.map(|power| InteriorBounds::new(power, power)),
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
        OPPOSITE_PAIR_SOURCE.map(|power| InteriorBounds::new(power, power)),
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
