//! Generated recurrences for the two-inactive-numerator decorated-path branch.
//!
//! The first two cells use the symmetry-oriented source faces
//! `J(0,0,1,N,2,1)` and `J(0,0,2,N,1,1)`, `N < 0`, so their targets are the
//! exact canonical S4 placement classes reached by adding an inactive
//! numerator.  The third cell owns `J(0,0,1,N,1,2)`, `N <= -2`, using a
//! shifted source parameter so the target reaches `i64::MIN` without
//! arithmetic overflow.  Every compact production span is reprojected from
//! a complete generated ordinary-K6 search.

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

pub(super) const PATH_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
pub(super) const PAIR_REPLAY_ANCHOR: [i64; 6] = [-2, -2, 3, -2, 3, 3];
pub(super) const OPPOSITE_INACTIVE_PAIR_PIVOT: [i64; 6] = [0, -1, 0, 0, 0, 0];
pub(super) const ADJACENT_INACTIVE_PAIR_PIVOT: [i64; 6] = [-1, 0, 0, 0, 0, 0];
pub(super) const SHIFTED_DOT_PIVOT: [i64; 6] = [0, 0, 0, -1, 0, 1];
pub(super) const FREE_POSITION: usize = 3;
const PAIR_SEARCH_DEPTH: usize = 1;
const SHIFTED_DOT_SEARCH_DEPTH: usize = 0;

pub(super) struct DecoratedPathInactivePairSelectionWitness {
    pub(super) opposite_complete_sources: SourceViewBatch,
    pub(super) pair_complete_sources: SourceViewBatch,
    pub(super) opposite_complete_rule: ParametricRule,
    pub(super) adjacent_complete_rule: ParametricRule,
    pub(super) opposite_machine_safe_sources: SourceViewBatch,
    pub(super) pair_machine_safe_sources: SourceViewBatch,
    pub(super) opposite_machine_safe_rule: ParametricRule,
    pub(super) adjacent_machine_safe_rule: ParametricRule,
    pub(super) shifted_dot_complete_sources: SourceViewBatch,
    pub(super) shifted_dot_complete_rule: ParametricRule,
    pub(super) shifted_dot_machine_safe_sources: SourceViewBatch,
    pub(super) shifted_dot_machine_safe_rule: ParametricRule,
}

pub(super) struct DecoratedPathInactivePairBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) opposite: RuleCell,
    pub(super) adjacent: RuleCell,
    pub(super) shifted_dot: RuleCell,
    pub(super) opposite_machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) pair_machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) opposite_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) adjacent_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) shifted_dot_machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) shifted_dot_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<DecoratedPathInactivePairSelectionWitness>,
}

pub(super) fn derive_decorated_path_inactive_pair_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell, RuleCell), ArtifactError> {
    let build = derive_decorated_path_inactive_pair_build(false)?;
    Ok((
        build.context,
        build.opposite,
        build.adjacent,
        build.shifted_dot,
    ))
}

pub(super) fn derive_decorated_path_inactive_pair_build(
    retain_selection_witness: bool,
) -> Result<DecoratedPathInactivePairBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;

    let pair_search = search(PAIR_SEARCH_DEPTH)?;
    let opposite_complete_sources = project_opposite_sources(
        &generator,
        &completed,
        pair_search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    let opposite_complete_rule = derive_rule(
        &generator,
        &opposite_complete_sources,
        &OPPOSITE_INACTIVE_PAIR_PIVOT,
    )?;
    let opposite_safe_domain = opposite_source_domain(i64::MIN)?;
    let opposite_machine_safe_complete_source_ordinals =
        machine_safe_ordinals(&opposite_complete_sources, &opposite_safe_domain);
    let opposite_machine_safe_sources = project_opposite_sources(
        &generator,
        &completed,
        pair_search.offsets().iter().cloned(),
        Some(&opposite_machine_safe_complete_source_ordinals),
        i64::MIN,
        &canonicalizer,
        &zero_sectors,
    )?;
    let opposite_machine_safe_rule = derive_rule(
        &generator,
        &opposite_machine_safe_sources,
        &OPPOSITE_INACTIVE_PAIR_PIVOT,
    )?;
    let opposite_selected_complete_source_ordinals = map_selected_ordinals(
        &opposite_machine_safe_rule,
        &opposite_machine_safe_complete_source_ordinals,
    );
    let opposite = build_opposite_cell(
        &generator,
        &completed,
        &pair_search,
        &opposite_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;

    let pair_complete_sources = project_pair_sources(
        &generator,
        &completed,
        pair_search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    let adjacent_complete_rule = derive_rule(
        &generator,
        &pair_complete_sources,
        &ADJACENT_INACTIVE_PAIR_PIVOT,
    )?;
    let pair_safe_domain = pair_source_domain(i64::MIN)?;
    let pair_machine_safe_complete_source_ordinals =
        machine_safe_ordinals(&pair_complete_sources, &pair_safe_domain);
    let pair_machine_safe_sources = project_pair_sources(
        &generator,
        &completed,
        pair_search.offsets().iter().cloned(),
        Some(&pair_machine_safe_complete_source_ordinals),
        i64::MIN,
        &canonicalizer,
        &zero_sectors,
    )?;
    let adjacent_machine_safe_rule = derive_rule(
        &generator,
        &pair_machine_safe_sources,
        &ADJACENT_INACTIVE_PAIR_PIVOT,
    )?;
    let adjacent_selected_complete_source_ordinals = map_selected_ordinals(
        &adjacent_machine_safe_rule,
        &pair_machine_safe_complete_source_ordinals,
    );
    let adjacent = build_pair_cell(
        &generator,
        &completed,
        &pair_search,
        &adjacent_selected_complete_source_ordinals,
        &ADJACENT_INACTIVE_PAIR_PIVOT,
        &canonicalizer,
        &zero_sectors,
    )?;

    let shifted_dot_search = search(SHIFTED_DOT_SEARCH_DEPTH)?;
    let shifted_dot_complete_sources = project_shifted_dot_sources(
        &generator,
        &completed,
        shifted_dot_search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    let shifted_dot_complete_rule = derive_rule(
        &generator,
        &shifted_dot_complete_sources,
        &SHIFTED_DOT_PIVOT,
    )?;
    let shifted_dot_safe_domain = shifted_dot_source_domain(i64::MIN + 1)?;
    let shifted_dot_machine_safe_complete_source_ordinals =
        machine_safe_ordinals(&shifted_dot_complete_sources, &shifted_dot_safe_domain);
    let shifted_dot_machine_safe_sources = project_shifted_dot_sources(
        &generator,
        &completed,
        shifted_dot_search.offsets().iter().cloned(),
        Some(&shifted_dot_machine_safe_complete_source_ordinals),
        i64::MIN + 1,
        &canonicalizer,
        &zero_sectors,
    )?;
    let shifted_dot_machine_safe_rule = derive_rule(
        &generator,
        &shifted_dot_machine_safe_sources,
        &SHIFTED_DOT_PIVOT,
    )?;
    let shifted_dot_selected_complete_source_ordinals = map_selected_ordinals(
        &shifted_dot_machine_safe_rule,
        &shifted_dot_machine_safe_complete_source_ordinals,
    );
    let shifted_dot_sources = project_shifted_dot_sources(
        &generator,
        &completed,
        shifted_dot_search.offsets().iter().cloned(),
        Some(&shifted_dot_selected_complete_source_ordinals),
        i64::MIN + 1,
        &canonicalizer,
        &zero_sectors,
    )?;
    let shifted_dot_rule = derive_rule(&generator, &shifted_dot_sources, &SHIFTED_DOT_PIVOT)?;
    let shifted_dot_application =
        application_domain(&shifted_dot_rule, shifted_dot_source_bounds(i64::MIN + 1))?;
    let shifted_dot = RuleCell::try_refined(
        generator.context(),
        shifted_dot_rule,
        shifted_dot_sources,
        shifted_dot_application,
        fixed_shifted_dot_source_face(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness =
        retain_selection_witness.then_some(DecoratedPathInactivePairSelectionWitness {
            opposite_complete_sources,
            pair_complete_sources,
            opposite_complete_rule,
            adjacent_complete_rule,
            opposite_machine_safe_sources,
            pair_machine_safe_sources,
            opposite_machine_safe_rule,
            adjacent_machine_safe_rule,
            shifted_dot_complete_sources,
            shifted_dot_complete_rule,
            shifted_dot_machine_safe_sources,
            shifted_dot_machine_safe_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(DecoratedPathInactivePairBuild {
        context,
        opposite,
        adjacent,
        shifted_dot,
        opposite_machine_safe_complete_source_ordinals,
        pair_machine_safe_complete_source_ordinals,
        opposite_selected_complete_source_ordinals,
        adjacent_selected_complete_source_ordinals,
        shifted_dot_machine_safe_complete_source_ordinals,
        shifted_dot_selected_complete_source_ordinals,
        selection_witness,
    })
}

fn build_opposite_cell(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    search: &SectorSearchDiamond,
    selected_complete_source_ordinals: &[usize],
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<RuleCell, ArtifactError> {
    let sources = project_opposite_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        Some(selected_complete_source_ordinals),
        i64::MIN,
        canonicalizer,
        zero_sectors,
    )?;
    let rule = derive_rule(generator, &sources, &OPPOSITE_INACTIVE_PAIR_PIVOT)?;
    let application = application_domain(&rule, opposite_source_bounds(i64::MIN))?;
    Ok(RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed_opposite_source_face(),
        [],
        RuleCellLimits::default(),
    )?)
}

fn build_pair_cell(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    search: &SectorSearchDiamond,
    selected_complete_source_ordinals: &[usize],
    pivot: &[i64; 6],
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<RuleCell, ArtifactError> {
    let sources = project_pair_sources(
        generator,
        completed,
        search.offsets().iter().cloned(),
        Some(selected_complete_source_ordinals),
        i64::MIN,
        canonicalizer,
        zero_sectors,
    )?;
    let rule = derive_rule(generator, &sources, pivot)?;
    let application = application_domain(&rule, pair_source_bounds(i64::MIN))?;
    Ok(RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        fixed_pair_source_face(),
        [],
        RuleCellLimits::default(),
    )?)
}

fn derive_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &PAIR_REPLAY_ANCHOR,
        pivot,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_pair_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: Option<&[usize]>,
    lower: i64,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    project_sources(
        generator,
        completed,
        translations,
        ordinals,
        pair_source_domain(lower)?,
        &fixed_pair_source_face(),
        canonicalizer,
        zero_sectors,
    )
}

fn project_opposite_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: Option<&[usize]>,
    lower: i64,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    project_sources(
        generator,
        completed,
        translations,
        ordinals,
        opposite_source_domain(lower)?,
        &fixed_opposite_source_face(),
        canonicalizer,
        zero_sectors,
    )
}

fn project_shifted_dot_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: Option<&[usize]>,
    lower: i64,
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    project_sources(
        generator,
        completed,
        translations,
        ordinals,
        shifted_dot_source_domain(lower)?,
        &fixed_shifted_dot_source_face(),
        canonicalizer,
        zero_sectors,
    )
}

fn project_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: Option<&[usize]>,
    domain: SectorInteriorDomain,
    fixed: &[FixedIndexRestriction],
    canonicalizer: &crate::sector::symmetry::Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        translations,
        TranslatedSourceLimits::default(),
    )?;
    match ordinals {
        Some(ordinals) => Ok(SourceViewBatch::try_project_residual(
            translated,
            ordinals,
            generator.context(),
            domain,
            fixed.iter().copied(),
            canonicalizer,
            zero_sectors,
            RuleCellLimits::default(),
        )?),
        None => Ok(SourceViewBatch::try_project_complete_residual(
            translated,
            generator.context(),
            domain,
            fixed.iter().copied(),
            canonicalizer,
            zero_sectors,
            RuleCellLimits::default(),
        )?),
    }
}

fn machine_safe_ordinals(sources: &SourceViewBatch, domain: &SectorInteriorDomain) -> Box<[usize]> {
    sources
        .relations()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, relation)| {
            relation
                .terms()
                .keys()
                .all(|shift| {
                    domain
                        .bounds()
                        .iter()
                        .zip(shift.values())
                        .all(|(bounds, &delta)| {
                            bounds.lower().checked_add(delta).is_some()
                                && bounds.upper().checked_add(delta).is_some()
                        })
                })
                .then_some(ordinal)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn map_selected_ordinals(rule: &ParametricRule, safe_ordinals: &[usize]) -> Box<[usize]> {
    rule.source_combination()
        .iter()
        .map(|source| safe_ordinals[source.source_ordinal()])
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(PATH_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn pair_search_depth() -> usize {
    PAIR_SEARCH_DEPTH
}

pub(super) const fn shifted_dot_search_depth() -> usize {
    SHIFTED_DOT_SEARCH_DEPTH
}

#[cfg(test)]
pub(super) fn derive_pair_candidate(
    depth: usize,
    pivot: &[i64; 6],
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _) = complete_ordinary_sources(&generator)?;
    let search = search(depth)?;
    let sources = project_pair_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_rule(&generator, &sources, pivot)
}

#[cfg(test)]
pub(super) fn derive_opposite_candidate(depth: usize) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _) = complete_ordinary_sources(&generator)?;
    let search = search(depth)?;
    let sources = project_opposite_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_rule(&generator, &sources, &OPPOSITE_INACTIVE_PAIR_PIVOT)
}

pub(super) fn fixed_pair_source_face() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 0),
        FixedIndexRestriction::new(2, 2),
        FixedIndexRestriction::new(4, 1),
        FixedIndexRestriction::new(5, 1),
    ]
}

pub(super) fn fixed_opposite_source_face() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 0),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(4, 2),
        FixedIndexRestriction::new(5, 1),
    ]
}

pub(super) fn fixed_shifted_dot_source_face() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 0),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(4, 1),
        FixedIndexRestriction::new(5, 1),
    ]
}

fn pair_source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(2, 2),
        InteriorBounds::new(lower, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

fn shifted_dot_source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

fn opposite_source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, -1),
        InteriorBounds::new(2, 2),
        InteriorBounds::new(1, 1),
    ]
}

fn pair_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&PATH_SECTOR)?,
        pair_source_bounds(lower),
    )?)
}

fn shifted_dot_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&PATH_SECTOR)?,
        shifted_dot_source_bounds(lower),
    )?)
}

fn opposite_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&PATH_SECTOR)?,
        opposite_source_bounds(lower),
    )?)
}

fn application_domain(
    rule: &ParametricRule,
    source_bounds: [InteriorBounds; 6],
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&PATH_SECTOR)?,
        source_bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}
