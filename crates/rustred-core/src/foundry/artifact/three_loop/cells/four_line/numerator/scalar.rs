//! Exact negative inactive-power recurrences on the scalar four-line face.
//!
//! A complete depth-one search first derives the endpoint at inactive power
//! `-1` and independently selects the five generated rows needed by the
//! negative bulk.  The selected rows are then retranslated and residually
//! projected on `J(0,1,1,1,1,n)` over the full representable assignment box
//! `i64::MIN + 1 <= n <= -1`.  The resulting bulk cell owns target powers
//! through `i64::MIN`; its pinched numerator descendants remain explicit
//! closure obligations.

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
use super::super::corner::{derive_exact_corner_cell, project_complete_exact_corner_sources};

pub(super) const INACTIVE_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 0, 0, 0, -1];
pub(super) const BULK_REPLAY_ANCHOR: [i64; 6] = [0, 3, 3, 3, 3, -2];
const SEARCH_DEPTH: usize = 1;

pub(super) struct InactiveNumeratorSelectionWitness {
    pub(super) sources: SourceViewBatch,
    pub(super) rule: ParametricRule,
}

pub(super) struct InactiveNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) bulk: RuleCell,
    pub(super) endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) bulk_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) bulk_selection_witness: Option<InactiveNumeratorSelectionWitness>,
}

pub(in super::super) fn derive_inactive_numerator_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_inactive_numerator_build(false)?;
    Ok((build.context, build.endpoint, build.bulk))
}

pub(super) fn derive_inactive_numerator_build(
    retain_bulk_selection_witness: bool,
) -> Result<InactiveNumeratorBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?;

    // The exact scalar corner owns the endpoint selection and its complete
    // generated depth-one provenance.
    let endpoint_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    let endpoint =
        derive_exact_corner_cell(&generator, endpoint_sources, &INACTIVE_NUMERATOR_PIVOT)?;
    let endpoint_selected_complete_source_ordinals = selected_source_ordinals(endpoint.rule());

    // The complete 63-row free-face projection needs two cells of lower
    // headroom.  It is used only to discover the exact five-row selection;
    // the selected rows are independently projected below with one-cell
    // headroom, which is sufficient for a target at i64::MIN.
    let complete_bulk_sources = project_complete_bulk_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_bulk_rule = derive_bulk_rule(&generator, &complete_bulk_sources)?;
    let bulk_selected_complete_source_ordinals = selected_source_ordinals(&complete_bulk_rule);
    let bulk_selection_witness =
        retain_bulk_selection_witness.then_some(InactiveNumeratorSelectionWitness {
            sources: complete_bulk_sources,
            rule: complete_bulk_rule,
        });

    let bulk_sources = project_selected_bulk_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &bulk_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let bulk_rule = derive_bulk_rule(&generator, &bulk_sources)?;
    let bulk_application = bulk_application_domain(&bulk_rule)?;
    let bulk = RuleCell::try_refined(
        generator.context(),
        bulk_rule,
        bulk_sources,
        bulk_application,
        fixed_scalar_face(),
        [],
        RuleCellLimits::default(),
    )?;

    let context = generator.context().clone();
    drop(generator);
    Ok(InactiveNumeratorBuild {
        context,
        endpoint,
        bulk,
        endpoint_selected_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        bulk_selection_witness,
    })
}

fn derive_bulk_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &BULK_REPLAY_ANCHOR,
        &INACTIVE_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_complete_bulk_sources(
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
        bulk_source_domain(i64::MIN + 2)?,
        fixed_scalar_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn project_selected_bulk_sources(
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
        bulk_source_domain(i64::MIN + 1)?,
        fixed_scalar_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn selected_source_ordinals(rule: &ParametricRule) -> Box<[usize]> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

pub(super) const fn inactive_numerator_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_scalar_face() -> [FixedIndexRestriction; 5] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, FOUR_LINE_SECTOR[position]))
}

fn bulk_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(lower, -1),
        ],
    )?)
}

fn bulk_application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(i64::MIN + 1, -1),
        ],
        rule.pivot().values(),
        &rhs,
    )?)
}
