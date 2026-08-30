//! Exact endpoint generated for the decorated child of the bridge-dot lane.
//!
//! The factorized bridge-dot bulk exposes `J(-1,0,1,0,2,1)`.  This module
//! owns exactly that `S4` orbit.  A complete untranslated span of the nine
//! ordinary K6 rows selects two generated relations; production reprojects
//! only those selected rows.  Both children already have exact owners, so an
//! unproved bulk extension is deliberately outside this endpoint cell.

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

pub(super) const BRIDGE_DESCENDANT_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
pub(super) const BRIDGE_DESCENDANT_TARGET_SHIFT: [i64; 6] = [-1, 0, 0, 0, 1, 0];
const SEARCH_DEPTH: usize = 0;

pub(super) struct BridgeDescendantSelectionWitness {
    pub(super) complete_sources: SourceViewBatch,
    pub(super) complete_rule: ParametricRule,
}

pub(super) struct BridgeDescendantEndpointBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<BridgeDescendantSelectionWitness>,
}

pub(super) fn derive_bridge_descendant_dot_numerator_endpoint()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_bridge_descendant_endpoint_build(false)?;
    Ok((build.context, build.endpoint))
}

pub(super) fn derive_bridge_descendant_endpoint_build(
    retain_selection_witness: bool,
) -> Result<BridgeDescendantEndpointBuild, ArtifactError> {
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
        fixed_endpoint(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness = retain_selection_witness.then_some(BridgeDescendantSelectionWitness {
        complete_sources,
        complete_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(BridgeDescendantEndpointBuild {
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
        &BRIDGE_DESCENDANT_SECTOR,
        &BRIDGE_DESCENDANT_TARGET_SHIFT,
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

fn depth_search() -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(BRIDGE_DESCENDANT_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn bridge_descendant_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_endpoint() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, BRIDGE_DESCENDANT_SECTOR[position])
    })
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&BRIDGE_DESCENDANT_SECTOR)?,
        BRIDGE_DESCENDANT_SECTOR.map(|power| InteriorBounds::new(power, power)),
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
        Mask::try_from_indices(&BRIDGE_DESCENDANT_SECTOR)?,
        BRIDGE_DESCENDANT_SECTOR.map(|power| InteriorBounds::new(power, power)),
        rule.pivot().values(),
        &rhs,
    )?)
}
