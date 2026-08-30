//! Exact bulk recurrence for one dot with a deeper inactive numerator.
//!
//! The existing mixed-numerator cell owns the `N = -1` boundary across its
//! positive active-power box.  This disjoint cell owns the scalar active face
//! `J(0,1,1,1,2,N)` for `N <= -2`.  It is generated from the complete nine
//! ordinary depth-zero K6 rows, then the selected rows are independently
//! projected over the full representable source interval.

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

pub(super) const DOTTED_NEGATIVE_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 0, 0, 1, -1];
pub(super) const BULK_REPLAY_ANCHOR: [i64; 6] = [0, 3, 3, 3, 3, -2];
pub(super) const FREE_POSITION: usize = 5;
const SEARCH_DEPTH: usize = 0;

pub(super) struct DottedNegativeNumeratorSelectionWitness {
    pub(super) complete_sources: SourceViewBatch,
    pub(super) complete_rule: ParametricRule,
}

pub(super) struct DottedNegativeNumeratorBulkBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) bulk: RuleCell,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<DottedNegativeNumeratorSelectionWitness>,
}

pub(in super::super) fn derive_dotted_negative_numerator_bulk()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_dotted_negative_numerator_bulk_build(false)?;
    Ok((build.context, build.bulk))
}

pub(super) fn derive_dotted_negative_numerator_bulk_build(
    retain_selection_witness: bool,
) -> Result<DottedNegativeNumeratorBulkBuild, ArtifactError> {
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
    let complete_rule = derive_bulk_rule(&generator, &complete_sources)?;
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
    let rule = derive_bulk_rule(&generator, &selected_sources)?;
    let application = application_domain(&rule)?;
    let bulk = RuleCell::try_refined(
        generator.context(),
        rule,
        selected_sources,
        application,
        fixed_scalar_source_face(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness =
        retain_selection_witness.then_some(DottedNegativeNumeratorSelectionWitness {
            complete_sources,
            complete_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(DottedNegativeNumeratorBulkBuild {
        context,
        bulk,
        selected_complete_source_ordinals,
        selection_witness,
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
        &DOTTED_NEGATIVE_NUMERATOR_PIVOT,
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
        source_domain(i64::MIN + 2)?,
        fixed_scalar_source_face(),
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
        source_domain(i64::MIN + 1)?,
        fixed_scalar_source_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn depth_search() -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn dotted_negative_numerator_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_scalar_source_face() -> [FixedIndexRestriction; 5] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, FOUR_LINE_SECTOR[position]))
}

fn source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        scalar_source_bounds(lower),
    )?)
}

fn scalar_source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, -1),
    ]
}

fn application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        scalar_source_bounds(i64::MIN + 1),
        rule.pivot().values(),
        &rhs,
    )?)
}
