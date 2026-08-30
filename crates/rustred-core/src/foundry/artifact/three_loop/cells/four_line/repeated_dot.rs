//! Exact parametric recurrence on the canonical four-line repeated-dot ray.
//!
//! The source span is generated, not authored: it is the complete depth-two
//! same-sector diamond (28 translations) of all nine ordinary K6 IBP rows.
//! Residual projection fixes the four-line face and leaves only the selected
//! active-edge power free. The structural application box covers
//! `J(0,1,1,1,n,0)` with `n >= 3`. Exact tests prove that no retained guard
//! becomes the zero polynomial in `d` at any positive free index; concrete
//! exceptional dimensions remain guarded. This is one discovery cell, not a
//! K6 closure claim.

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

pub(super) const REPEATED_DOT_TARGET_SHIFT: [i64; 6] = [0, 0, 0, 0, 2, 0];
const RAY_ANCHOR: [i64; 6] = FOUR_LINE_SECTOR;
const RAY_FREE_POSITION: usize = 4;
const SEARCH_DEPTH: usize = 2;

pub(super) fn derive_repeated_dot_ray_cell()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _source_count) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(RAY_ANCHOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?;
    let translated = generator.translate_completed_source_rows(
        &completed,
        search.offsets().iter().cloned(),
        TranslatedSourceLimits::default(),
    )?;
    let sources = SourceViewBatch::try_project_complete_residual(
        translated,
        generator.context(),
        ray_source_domain()?,
        fixed_ray_indices(),
        &canonicalizer,
        &zero_sectors,
        RuleCellLimits::default(),
    )?;
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &RAY_ANCHOR,
        &REPEATED_DOT_TARGET_SHIFT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?;
    let application = ray_application_domain(&rule)?;
    let context = generator.context().clone();
    let cell = RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed_ray_indices(),
        [],
        RuleCellLimits::default(),
    )?;
    drop(generator);
    Ok((context, cell))
}

pub(super) const fn repeated_dot_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) const fn repeated_dot_free_position() -> usize {
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
