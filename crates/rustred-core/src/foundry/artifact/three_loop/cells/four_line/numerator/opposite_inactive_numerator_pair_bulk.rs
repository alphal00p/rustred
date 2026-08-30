//! Machine-wide continuation of the opposite inactive-numerator pair.
//!
//! This cell owns `J(-1,1,1,1,1,N)` for `N <= -2`.  Its identity is
//! discovered from the complete depth-one ordinary K6 span.  Production is
//! independently reprojected from the selected generated rows over the full
//! representable source interval; no recurrence coefficients are authored.

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

pub(super) const OPPOSITE_PAIR_BULK_PIVOT: [i64; 6] = [0, 0, 0, 0, 0, -1];
pub(super) const OPPOSITE_PAIR_BULK_REPLAY_ANCHOR: [i64; 6] = [-1, 3, 3, 3, 3, -2];
pub(super) const FREE_POSITION: usize = 5;
const SEARCH_DEPTH: usize = 1;

pub(super) struct OppositePairBulkSelectionWitness {
    pub(super) complete_sources: SourceViewBatch,
    pub(super) complete_rule: ParametricRule,
    pub(super) machine_safe_sources: SourceViewBatch,
    pub(super) machine_safe_rule: ParametricRule,
}

pub(super) struct OppositePairBulkBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) bulk: RuleCell,
    pub(super) machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<OppositePairBulkSelectionWitness>,
}

pub(in super::super) fn derive_opposite_inactive_numerator_pair_bulk()
-> Result<(IndexedCoefficientContext, RuleCell), ArtifactError> {
    let build = derive_opposite_pair_bulk_build(false)?;
    Ok((build.context, build.bulk))
}

pub(super) fn derive_opposite_pair_bulk_build(
    retain_selection_witness: bool,
) -> Result<OppositePairBulkBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search()?;

    let complete_sources = project_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_rule = derive_bulk_rule(&generator, &complete_sources)?;

    let safe_domain = source_domain(i64::MIN + 1)?;
    let machine_safe_complete_source_ordinals = complete_sources
        .relations()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, relation)| {
            relation_is_machine_representable(relation, &safe_domain).then_some(ordinal)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let machine_safe_sources = project_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        Some(&machine_safe_complete_source_ordinals),
        i64::MIN + 1,
        &canonicalizer,
        &zero_sectors,
    )?;
    let machine_safe_rule = derive_bulk_rule(&generator, &machine_safe_sources)?;
    let selected_complete_source_ordinals = machine_safe_rule
        .source_combination()
        .iter()
        .map(|source| machine_safe_complete_source_ordinals[source.source_ordinal()])
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let selected_sources = project_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        Some(&selected_complete_source_ordinals),
        i64::MIN + 1,
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
        fixed_source_face(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness = retain_selection_witness.then_some(OppositePairBulkSelectionWitness {
        complete_sources,
        complete_rule,
        machine_safe_sources,
        machine_safe_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(OppositePairBulkBuild {
        context,
        bulk,
        machine_safe_complete_source_ordinals,
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
        &OPPOSITE_PAIR_BULK_REPLAY_ANCHOR,
        &OPPOSITE_PAIR_BULK_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: Option<&[usize]>,
    lower: i64,
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
            source_domain(lower)?,
            fixed_source_face(),
            canonicalizer,
            zero_sectors,
            RuleCellLimits::default(),
        )?),
        None => Ok(SourceViewBatch::try_project_complete_residual(
            translated,
            generator.context(),
            source_domain(lower)?,
            fixed_source_face(),
            canonicalizer,
            zero_sectors,
            RuleCellLimits::default(),
        )?),
    }
}

fn relation_is_machine_representable(
    relation: &crate::identity::ParametricRelation,
    domain: &SectorInteriorDomain,
) -> bool {
    relation.terms().keys().all(|shift| {
        domain
            .bounds()
            .iter()
            .zip(shift.values())
            .all(|(bounds, &delta)| {
                bounds.lower().checked_add(delta).is_some()
                    && bounds.upper().checked_add(delta).is_some()
            })
    })
}

fn depth_search() -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn opposite_pair_bulk_search_depth() -> usize {
    SEARCH_DEPTH
}

#[cfg(test)]
pub(super) fn derive_opposite_pair_bulk_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(FOUR_LINE_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?;
    let sources = project_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        None,
        i64::MIN + 2,
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_bulk_rule(&generator, &sources)
}

pub(super) fn fixed_source_face() -> [FixedIndexRestriction; 5] {
    std::array::from_fn(|position| FixedIndexRestriction::new(position, [-1, 1, 1, 1, 1][position]))
}

fn source_bounds(lower: i64) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(-1, -1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(lower, -1),
    ]
}

fn source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        source_bounds(lower),
    )?)
}

fn application_domain(rule: &ParametricRule) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FOUR_LINE_SECTOR)?,
        source_bounds(i64::MIN + 1),
        rule.pivot().values(),
        &rhs,
    )?)
}
