//! Generated negative-power recurrences on the undotted three-line path.
//!
//! This owner is distinct from factorization: only the scalar endpoint
//! `J(0,0,1,0,1,1)` is a registered product terminal.  A complete depth-one
//! source span derives the numerator endpoint.  For the bulk, authenticated
//! residual routes select every generated row representable on the intended
//! machine-wide source box; exact elimination then chooses five rows from
//! that generated safe span.  Retranslating and reprojecting those rows proves
//! coverage through target power `i64::MIN` without authored algebra.

use crate::algebra::IndexedCoefficientContext;
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{
    FixedIndexRestriction, ResidualTermDisposition, RuleCell, RuleCellLimits, SourceViewBatch,
    SourceViewConstruction,
};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_interior_rule_for_target,
    derive_sector_monotone_rule_for_target,
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

pub(super) const UNDOTTED_PATH_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
pub(super) const UNDOTTED_PATH_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 0, -1, 0, 0];
pub(super) const UNDOTTED_BULK_REPLAY_ANCHOR: [i64; 6] = [0, 0, 2, -2, 2, 2];
const SEARCH_DEPTH: usize = 1;

pub(super) struct UndottedPathNumeratorSelectionWitness {
    pub(super) complete_endpoint_sources: SourceViewBatch,
    pub(super) complete_endpoint_rule: ParametricRule,
    pub(super) complete_free_sources: SourceViewBatch,
    pub(super) machine_safe_sources: SourceViewBatch,
    pub(super) machine_safe_rule: ParametricRule,
}

pub(super) struct UndottedPathNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) bulk: RuleCell,
    pub(super) endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) bulk_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<UndottedPathNumeratorSelectionWitness>,
}

pub(super) fn derive_undotted_path_numerator_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_undotted_path_numerator_build(false)?;
    Ok((build.context, build.endpoint, build.bulk))
}

pub(super) fn derive_undotted_path_numerator_build(
    retain_selection_witness: bool,
) -> Result<UndottedPathNumeratorBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = depth_search(SEARCH_DEPTH)?;

    let complete_endpoint_sources = project_complete_endpoint_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_endpoint_rule = derive_endpoint_rule(&generator, &complete_endpoint_sources)?;
    let endpoint_selected_complete_source_ordinals =
        selected_source_ordinals(&complete_endpoint_rule);
    let endpoint_sources = project_selected_endpoint_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &endpoint_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let endpoint_rule = derive_endpoint_rule(&generator, &endpoint_sources)?;
    let endpoint_application = application_domain(
        &endpoint_rule,
        UNDOTTED_PATH_SECTOR.map(|power| InteriorBounds::new(power, power)),
    )?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        endpoint_rule,
        endpoint_sources,
        endpoint_application,
        fixed_endpoint(),
        [],
        RuleCellLimits::default(),
    )?;

    // The complete span is projected with exactly two cells of lower
    // headroom. Its authenticated routes then decide which generated rows
    // remain representable after tightening to one-cell headroom.
    let complete_free_sources = project_complete_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let safe_source_domain = free_source_domain(i64::MIN + 1)?;
    let machine_safe_complete_source_ordinals =
        machine_safe_source_ordinals(&complete_free_sources, &safe_source_domain)?;
    let machine_safe_sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &machine_safe_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let machine_safe_rule = derive_bulk_rule(&generator, &machine_safe_sources)?;
    let bulk_selected_complete_source_ordinals = machine_safe_rule
        .source_combination()
        .iter()
        .map(|contribution| machine_safe_complete_source_ordinals[contribution.source_ordinal()])
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let bulk_sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &bulk_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let bulk_rule = derive_bulk_rule(&generator, &bulk_sources)?;
    let bulk_application = application_domain(
        &bulk_rule,
        free_face_bounds(InteriorBounds::new(i64::MIN + 1, -1)),
    )?;
    let bulk = RuleCell::try_refined(
        generator.context(),
        bulk_rule,
        bulk_sources,
        bulk_application,
        fixed_free_face(),
        [],
        RuleCellLimits::default(),
    )?;

    let selection_witness =
        retain_selection_witness.then_some(UndottedPathNumeratorSelectionWitness {
            complete_endpoint_sources,
            complete_endpoint_rule,
            complete_free_sources,
            machine_safe_sources,
            machine_safe_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(UndottedPathNumeratorBuild {
        context,
        endpoint,
        bulk,
        endpoint_selected_complete_source_ordinals,
        machine_safe_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
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
        &UNDOTTED_PATH_SECTOR,
        &UNDOTTED_PATH_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn derive_bulk_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &UNDOTTED_BULK_REPLAY_ANCHOR,
        &UNDOTTED_PATH_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

fn project_complete_endpoint_sources(
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

fn project_complete_free_sources(
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
        free_source_domain(i64::MIN + 2)?,
        fixed_free_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn project_selected_endpoint_sources(
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

fn project_selected_free_sources(
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
        free_source_domain(i64::MIN + 1)?,
        fixed_free_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn machine_safe_source_ordinals(
    sources: &SourceViewBatch,
    intended_domain: &SectorInteriorDomain,
) -> Result<Box<[usize]>, ArtifactError> {
    let SourceViewConstruction::ResidualProjection(evidence) = sources.construction() else {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "undotted path machine-safety selection lacks residual routes",
        });
    };
    if evidence.term_projections().len() != sources.len()
        || intended_domain.arity() != evidence.domain().arity()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "undotted path machine-safety evidence has inconsistent arity",
        });
    }
    Ok(evidence
        .term_projections()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, terms)| {
            terms
                .iter()
                .all(|term| match term.disposition() {
                    ResidualTermDisposition::Routed {
                        projected_shift, ..
                    } => intended_domain
                        .bounds()
                        .iter()
                        .zip(projected_shift.iter())
                        .all(|(bounds, &delta)| {
                            bounds.lower().checked_add(delta).is_some()
                                && bounds.upper().checked_add(delta).is_some()
                        }),
                    ResidualTermDisposition::CoefficientZero
                    | ResidualTermDisposition::ProvedZero { .. } => true,
                })
                .then_some(ordinal)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

fn selected_source_ordinals(rule: &ParametricRule) -> Box<[usize]> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn application_domain(
    rule: &ParametricRule,
    bounds: [InteriorBounds; 6],
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&UNDOTTED_PATH_SECTOR)?,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}

fn depth_search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(UNDOTTED_PATH_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn undotted_path_numerator_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_free_face() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(1, 0),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(4, 1),
        FixedIndexRestriction::new(5, 1),
    ]
}

pub(super) fn fixed_endpoint() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, UNDOTTED_PATH_SECTOR[position])
    })
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&UNDOTTED_PATH_SECTOR)?,
        UNDOTTED_PATH_SECTOR.map(|power| InteriorBounds::new(power, power)),
    )?)
}

fn free_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&UNDOTTED_PATH_SECTOR)?,
        free_face_bounds(InteriorBounds::new(lower, -1)),
    )?)
}

fn free_face_bounds(free: InteriorBounds) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        InteriorBounds::new(0, 0),
        InteriorBounds::new(1, 1),
        free,
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

#[cfg(test)]
pub(super) fn derive_undotted_endpoint_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _) = complete_ordinary_sources(&generator)?;
    let search = depth_search(depth)?;
    let sources = project_complete_endpoint_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_endpoint_rule(&generator, &sources)
}

#[cfg(test)]
pub(super) fn derive_undotted_bulk_candidate(
    depth: usize,
) -> Result<ParametricRule, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _) = complete_ordinary_sources(&generator)?;
    let search = depth_search(depth)?;
    let complete = project_complete_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let safe = machine_safe_source_ordinals(&complete, &free_source_domain(i64::MIN + 1)?)?;
    let sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &safe,
        &canonicalizer,
        &zero_sectors,
    )?;
    derive_bulk_rule(&generator, &sources)
}
