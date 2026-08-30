//! Exact selected-source recurrence on one canonical mixed-dot ray.
//!
//! The source selection is generated rather than authored.  A complete
//! depth-three exact-corner search first selects 46 of the 756 translated
//! ordinary K6 rows.  Those rows are then independently retranslated and
//! projected on the face `J(0,1,1,1,n,0)`, leaving only `n` free.  A second
//! exact elimination derives the recurrence installed here.  The algebraic
//! recurrence and its guard proof cover one S4 orbit of
//! `J(0,1,2,2,N,0)` for every structural `N >= 3`.  Its concrete `RuleCell`
//! owns the representable interval `3 <= N <= i64::MAX - 1`; the
//! complementary orbit and its same-sector descendants remain obligations.

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_monotone_rule_for_target,
};
use crate::foundry::search::{SectorSearchDiamond, SectorSearchLimits};
use crate::identity::{ParametricIbpConfig, ParametricIbpGenerator, TranslatedSourceLimits};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::super::super::{canonical_family, canonical_s4, exact_zero_sectors};
use super::super::support::complete_ordinary_sources;
use super::FOUR_LINE_SECTOR;
use super::corner::{derive_exact_corner_cell, project_complete_exact_corner_sources};

pub(super) const MIXED_DOT_RAY_TARGET_SHIFT: [i64; 6] = [0, 0, 1, 1, 2, 0];
const RAY_FREE_POSITION: usize = 4;
const SEARCH_DEPTH: usize = 3;

pub(super) struct MixedDotRayBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) cell: RuleCell,
    pub(super) selection_witness: Option<RuleCell>,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) full_span_diagnosis: Option<Result<ParametricRule, ArtifactError>>,
}

/// Derive the discovery cell without performing the deliberately expensive
/// full-ray failure diagnosis retained by the exact test.
pub(super) fn derive_mixed_dot_ray_cell()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_mixed_dot_ray_build(false)?;
    Ok((build.context, build.cell))
}

/// Derive the cell and, when requested by the cohesive exact test, retain the
/// typed result of feeding the complete 756-row ray span directly to the
/// deterministic eliminator.
pub(super) fn derive_mixed_dot_ray_build(
    diagnose_complete_ray_span: bool,
) -> Result<MixedDotRayBuild, ArtifactError> {
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

    // The complete exact-corner result owns the source selection.  Only its
    // generated row ordinals cross into the independent free-index search.
    let singleton_sources = project_complete_exact_corner_sources(
        &generator,
        &completed,
        &canonicalizer,
        &zero_sectors,
        search.offsets().iter().cloned(),
    )?;
    let singleton =
        derive_exact_corner_cell(&generator, singleton_sources, &MIXED_DOT_RAY_TARGET_SHIFT)?;
    let selected_complete_source_ordinals = singleton
        .rule()
        .source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let selection_witness = diagnose_complete_ray_span.then_some(singleton);

    let selected_sources = project_ray_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        selected_sources.relations(),
        &FOUR_LINE_SECTOR,
        &MIXED_DOT_RAY_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let application = ray_application_domain(&rule)?;
    let cell = RuleCell::try_refined(
        generator.context(),
        rule,
        selected_sources,
        application,
        fixed_ray_indices(),
        [],
        RuleCellLimits::default(),
    )?;

    let full_span_diagnosis = if diagnose_complete_ray_span {
        let complete_sources = project_complete_ray_sources(
            &generator,
            &completed,
            search.offsets().iter().cloned(),
            &canonicalizer,
            &zero_sectors,
        )?;
        Some(
            derive_sector_monotone_rule_for_target(
                generator.context(),
                complete_sources.relations(),
                &FOUR_LINE_SECTOR,
                &MIXED_DOT_RAY_TARGET_SHIFT,
                OrderingPolicy::default(),
                ParametricRuleLimits::default(),
            )
            .map_err(ArtifactError::from),
        )
    } else {
        None
    };

    let context = generator.context().clone();
    drop(generator);
    Ok(MixedDotRayBuild {
        context,
        cell,
        selection_witness,
        selected_complete_source_ordinals,
        full_span_diagnosis,
    })
}

fn project_ray_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &crate::identity::CompletedIbpSourceRows,
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
        ray_source_domain()?,
        fixed_ray_indices(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn project_complete_ray_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &crate::identity::CompletedIbpSourceRows,
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
        ray_source_domain()?,
        fixed_ray_indices(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

pub(super) const fn mixed_dot_ray_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) const fn mixed_dot_ray_free_position() -> usize {
    RAY_FREE_POSITION
}

pub(super) fn fixed_ray_indices() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 1),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(3, 1),
        FixedIndexRestriction::new(5, 0),
    ]
}

fn ray_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(0, 0),
        ],
    )?)
}

fn ray_application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    let sector = Mask::try_from_indices(&FOUR_LINE_SECTOR)?;
    let maximal =
        SectorMonotoneDomain::try_maximal_for_rule(sector.clone(), rule.pivot().values(), &rhs)?;
    let mut bounds = maximal.bounds().to_vec();
    bounds[0] = InteriorBounds::new(0, 0);
    bounds[1] = InteriorBounds::new(1, 1);
    bounds[2] = InteriorBounds::new(1, 1);
    bounds[3] = InteriorBounds::new(1, 1);
    bounds[RAY_FREE_POSITION] = InteriorBounds::new(1, bounds[RAY_FREE_POSITION].upper());
    bounds[5] = InteriorBounds::new(0, 0);
    Ok(SectorMonotoneDomain::try_new_for_rule(
        sector,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}
