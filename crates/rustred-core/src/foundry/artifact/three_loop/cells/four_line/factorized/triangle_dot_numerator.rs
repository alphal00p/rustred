//! Generated numerator recurrences with the dot on the factorized triangle.
//!
//! Removing the two inactive K4 edges leaves a triangle with one attached
//! bridge.  This module owns two exact non-bridge placement classes.  The dot
//! opposite the bridge attachment is a machine-wide positive-power ray; the
//! dot on the K4 edge opposite the numerator is presently certified only at
//! powers two and three.  Every rule is selected from a complete translated
//! ordinary-source diamond and independently retranslated and reprojected for
//! production.  No recurrence coefficient is authored here.

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
use super::FACTORIZED_FACE_SECTOR;

pub(super) const BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, -1, 0, 0, 1, 0];
pub(super) const OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, -1, 0, 1, 0, 0];
pub(super) const OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, -1, 0, 2, 0, 0];
pub(super) const RAY_FREE_POSITION: usize = 4;
const RAY_SEARCH_DEPTH: usize = 1;
const ENDPOINT_SEARCH_DEPTH: usize = 1;
const REPEATED_ENDPOINT_SEARCH_DEPTH: usize = 2;

pub(super) struct FactorizedTriangleDotSelectionWitness {
    pub(super) complete_ray_sources: SourceViewBatch,
    pub(super) complete_ray_rule: ParametricRule,
    pub(super) complete_endpoint_sources: SourceViewBatch,
    pub(super) complete_endpoint_rule: ParametricRule,
    pub(super) complete_repeated_endpoint_sources: SourceViewBatch,
    pub(super) complete_repeated_endpoint_rule: ParametricRule,
}

pub(super) struct FactorizedTriangleDotNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) bridge_opposite_ray: RuleCell,
    pub(super) opposite_edge_endpoint: RuleCell,
    pub(super) opposite_edge_repeated_endpoint: RuleCell,
    pub(super) ray_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) repeated_endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<FactorizedTriangleDotSelectionWitness>,
}

pub(in super::super) fn derive_factorized_triangle_dot_numerator_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell, RuleCell), ArtifactError> {
    let build = derive_factorized_triangle_dot_numerator_build(false)?;
    Ok((
        build.context,
        build.bridge_opposite_ray,
        build.opposite_edge_endpoint,
        build.opposite_edge_repeated_endpoint,
    ))
}

pub(super) fn derive_factorized_triangle_dot_numerator_build(
    retain_selection_witness: bool,
) -> Result<FactorizedTriangleDotNumeratorBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;

    let ray_search = depth_search(RAY_SEARCH_DEPTH)?;
    let complete_ray_sources = project_complete_sources(
        &generator,
        &completed,
        ray_search.offsets().iter().cloned(),
        ray_source_domain()?,
        fixed_ray_source(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_ray_rule = derive_rule(
        &generator,
        &complete_ray_sources,
        &BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT,
    )?;
    let ray_selected_complete_source_ordinals = selected_source_ordinals(&complete_ray_rule);
    let ray_sources = project_selected_sources(
        &generator,
        &completed,
        ray_search.offsets().iter().cloned(),
        &ray_selected_complete_source_ordinals,
        ray_source_domain()?,
        fixed_ray_source(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let ray_rule = derive_rule(
        &generator,
        &ray_sources,
        &BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT,
    )?;
    let ray_application = ray_application_domain(&ray_rule)?;
    let bridge_opposite_ray = RuleCell::try_refined(
        generator.context(),
        ray_rule,
        ray_sources,
        ray_application,
        fixed_ray_source(),
        [],
        RuleCellLimits::default(),
    )?;

    let endpoint = derive_fixed_endpoint(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        ENDPOINT_SEARCH_DEPTH,
        &OPPOSITE_EDGE_DOT_NUMERATOR_PIVOT,
    )?;
    let repeated = derive_fixed_endpoint(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        REPEATED_ENDPOINT_SEARCH_DEPTH,
        &OPPOSITE_EDGE_REPEATED_DOT_NUMERATOR_PIVOT,
    )?;

    let selection_witness =
        retain_selection_witness.then_some(FactorizedTriangleDotSelectionWitness {
            complete_ray_sources,
            complete_ray_rule,
            complete_endpoint_sources: endpoint.complete_sources,
            complete_endpoint_rule: endpoint.complete_rule,
            complete_repeated_endpoint_sources: repeated.complete_sources,
            complete_repeated_endpoint_rule: repeated.complete_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(FactorizedTriangleDotNumeratorBuild {
        context,
        bridge_opposite_ray,
        opposite_edge_endpoint: endpoint.cell,
        opposite_edge_repeated_endpoint: repeated.cell,
        ray_selected_complete_source_ordinals,
        endpoint_selected_complete_source_ordinals: endpoint.selected_complete_source_ordinals,
        repeated_endpoint_selected_complete_source_ordinals: repeated
            .selected_complete_source_ordinals,
        selection_witness,
    })
}

struct GeneratedFixedEndpoint {
    complete_sources: SourceViewBatch,
    complete_rule: ParametricRule,
    selected_complete_source_ordinals: Box<[usize]>,
    cell: RuleCell,
}

fn derive_fixed_endpoint(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
    depth: usize,
    pivot: &[i64; 6],
) -> Result<GeneratedFixedEndpoint, ArtifactError> {
    let search = depth_search(depth)?;
    let complete_sources = project_complete_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        endpoint_source_domain()?,
        fixed_endpoint_source(),
        canonicalizer,
        zero_sectors,
    )?;
    let complete_rule = derive_rule(generator, &complete_sources, pivot)?;
    let selected_complete_source_ordinals = selected_source_ordinals(&complete_rule);
    let selected_sources = project_selected_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        &selected_complete_source_ordinals,
        endpoint_source_domain()?,
        fixed_endpoint_source(),
        canonicalizer,
        zero_sectors,
    )?;
    let selected_rule = derive_rule(generator, &selected_sources, pivot)?;
    let application = endpoint_application_domain(&selected_rule)?;
    let cell = RuleCell::try_refined(
        generator.context(),
        selected_rule,
        selected_sources,
        application,
        fixed_endpoint_source(),
        [],
        RuleCellLimits::default(),
    )?;
    Ok(GeneratedFixedEndpoint {
        complete_sources,
        complete_rule,
        selected_complete_source_ordinals,
        cell,
    })
}

fn derive_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &FACTORIZED_FACE_SECTOR,
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

fn depth_search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FACTORIZED_FACE_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn ray_search_depth() -> usize {
    RAY_SEARCH_DEPTH
}

pub(super) const fn endpoint_search_depth() -> usize {
    ENDPOINT_SEARCH_DEPTH
}

pub(super) const fn repeated_endpoint_search_depth() -> usize {
    REPEATED_ENDPOINT_SEARCH_DEPTH
}

#[cfg(test)]
pub(super) fn derive_complete_ray_candidate(depth: usize) -> Result<ParametricRule, ArtifactError> {
    derive_complete_candidate(depth, &BRIDGE_OPPOSITE_DOT_NUMERATOR_PIVOT, true)
}

#[cfg(test)]
pub(super) fn derive_complete_endpoint_candidate(
    depth: usize,
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    derive_complete_candidate(depth, pivot, false)
}

#[cfg(test)]
fn derive_complete_candidate(
    depth: usize,
    pivot: &[i64; 6],
    ray: bool,
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
        if ray {
            ray_source_domain()?
        } else {
            endpoint_source_domain()?
        },
        if ray {
            fixed_ray_source().into_iter().collect::<Vec<_>>()
        } else {
            fixed_endpoint_source().into_iter().collect::<Vec<_>>()
        },
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_rule(&generator, &sources, pivot)
}

pub(super) fn fixed_ray_source() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 0),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(3, 1),
        FixedIndexRestriction::new(5, 1),
    ]
}

pub(super) fn fixed_endpoint_source() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, FACTORIZED_FACE_SECTOR[position])
    })
}

fn ray_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    let mut bounds = FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power));
    bounds[RAY_FREE_POSITION] = InteriorBounds::new(1, i64::MAX);
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FACTORIZED_FACE_SECTOR)?,
        bounds,
    )?)
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FACTORIZED_FACE_SECTOR)?,
        FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power)),
    )?)
}

fn ray_application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let sector = Mask::try_from_indices(&FACTORIZED_FACE_SECTOR)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power));
    bounds[RAY_FREE_POSITION] = InteriorBounds::new(1, maximal.bounds()[RAY_FREE_POSITION].upper());
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
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
        Mask::try_from_indices(&FACTORIZED_FACE_SECTOR)?,
        FACTORIZED_FACE_SECTOR.map(|power| InteriorBounds::new(power, power)),
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
