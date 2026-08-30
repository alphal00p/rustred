//! Generated bridge-dot numerator recurrences on the factorized four-line face.
//!
//! The scalar sector `J(0,0,1,1,1,1)` factorizes, but a negative power on an
//! inactive edge is an explicit numerator obligation.  This module owns one
//! exact `S4` orbit, `J(0,N,2,1,1,1)` with `N < 0`, in which the dot lies on
//! the graph bridge.  Endpoint and bulk rules come only from the complete nine
//! ordinary K6 rows at the depth-zero source corner.  The bulk selection is
//! computed from authenticated residual routes that remain representable over
//! the full intended machine box, then independently reprojected.

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

pub(super) const FACTORIZED_FOUR_LINE_SECTOR: [i64; 6] = [0, 0, 1, 1, 1, 1];
pub(super) const BRIDGE_DOT_NUMERATOR_PIVOT: [i64; 6] = [0, -1, 1, 0, 0, 0];
pub(super) const BULK_REPLAY_ANCHOR: [i64; 6] = [0, -8, 8, 8, 8, 8];
pub(super) const FREE_POSITION: usize = 1;
const SEARCH_DEPTH: usize = 0;

pub(super) struct FactorizedBridgeDotSelectionWitness {
    pub(super) complete_endpoint_sources: SourceViewBatch,
    pub(super) complete_endpoint_rule: ParametricRule,
    pub(super) complete_free_sources: SourceViewBatch,
    pub(super) machine_safe_sources: SourceViewBatch,
    pub(super) machine_safe_rule: ParametricRule,
}

pub(super) struct FactorizedBridgeDotNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) bulk: RuleCell,
    pub(super) endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) bulk_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<FactorizedBridgeDotSelectionWitness>,
}

pub(super) fn derive_factorized_bridge_dot_numerator_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_factorized_bridge_dot_numerator_build(false)?;
    Ok((build.context, build.endpoint, build.bulk))
}

pub(super) fn derive_factorized_bridge_dot_numerator_build(
    retain_selection_witness: bool,
) -> Result<FactorizedBridgeDotNumeratorBuild, ArtifactError> {
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
    let endpoint_application = application_domain(&endpoint_rule, InteriorBounds::new(0, 0))?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        endpoint_rule,
        endpoint_sources,
        endpoint_application,
        fixed_endpoint(),
        [],
        RuleCellLimits::default(),
    )?;

    let complete_free_sources = project_complete_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let intended_source_domain = free_source_domain(i64::MIN + 1, 0)?;
    let machine_safe_complete_source_ordinals =
        machine_safe_source_ordinals(&complete_free_sources, &intended_source_domain)?;
    let machine_safe_sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &machine_safe_complete_source_ordinals,
        i64::MIN + 1,
        0,
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
        i64::MIN + 1,
        -1,
        &canonicalizer,
        &zero_sectors,
    )?;
    let bulk_rule = derive_bulk_rule(&generator, &bulk_sources)?;
    let bulk_application = application_domain(&bulk_rule, InteriorBounds::new(i64::MIN + 1, -1))?;
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
        retain_selection_witness.then_some(FactorizedBridgeDotSelectionWitness {
            complete_endpoint_sources,
            complete_endpoint_rule,
            complete_free_sources,
            machine_safe_sources,
            machine_safe_rule,
        });
    let context = generator.context().clone();
    drop(generator);
    Ok(FactorizedBridgeDotNumeratorBuild {
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
        &FACTORIZED_FOUR_LINE_SECTOR,
        &BRIDGE_DOT_NUMERATOR_PIVOT,
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
        &BULK_REPLAY_ANCHOR,
        &BRIDGE_DOT_NUMERATOR_PIVOT,
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
        free_source_domain(i64::MIN + 2, 0)?,
        fixed_free_face(),
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

#[allow(clippy::too_many_arguments)]
fn project_selected_free_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    translations: impl IntoIterator<Item = crate::identity::IntegralShift>,
    ordinals: &[usize],
    lower: i64,
    upper: i64,
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
        free_source_domain(lower, upper)?,
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
            detail: "factorized bridge-dot machine-safety selection lacks residual routes",
        });
    };
    if evidence.term_projections().len() != sources.len()
        || intended_domain.arity() != evidence.domain().arity()
    {
        return Err(ArtifactError::InvalidReplayEvidence {
            detail: "factorized bridge-dot machine-safety evidence has inconsistent arity",
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

fn depth_search(depth: usize) -> Result<SectorSearchDiamond, ArtifactError> {
    Ok(SectorSearchDiamond::try_new(
        IntegralKey::try_new(FACTORIZED_FOUR_LINE_SECTOR)?,
        depth,
        SectorSearchLimits::default(),
    )?)
}

pub(super) const fn factorized_bridge_dot_search_depth() -> usize {
    SEARCH_DEPTH
}

pub(super) fn fixed_endpoint() -> [FixedIndexRestriction; 6] {
    std::array::from_fn(|position| {
        FixedIndexRestriction::new(position, FACTORIZED_FOUR_LINE_SECTOR[position])
    })
}

pub(super) fn fixed_free_face() -> [FixedIndexRestriction; 5] {
    [
        FixedIndexRestriction::new(0, 0),
        FixedIndexRestriction::new(2, 1),
        FixedIndexRestriction::new(3, 1),
        FixedIndexRestriction::new(4, 1),
        FixedIndexRestriction::new(5, 1),
    ]
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FACTORIZED_FOUR_LINE_SECTOR)?,
        FACTORIZED_FOUR_LINE_SECTOR.map(|power| InteriorBounds::new(power, power)),
    )?)
}

fn free_source_domain(lower: i64, upper: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&FACTORIZED_FOUR_LINE_SECTOR)?,
        free_face_bounds(InteriorBounds::new(lower, upper)),
    )?)
}

fn free_face_bounds(free: InteriorBounds) -> [InteriorBounds; 6] {
    [
        InteriorBounds::new(0, 0),
        free,
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
        InteriorBounds::new(1, 1),
    ]
}

fn application_domain(
    rule: &ParametricRule,
    free: InteriorBounds,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&FACTORIZED_FOUR_LINE_SECTOR)?,
        free_face_bounds(free),
        rule.pivot().values(),
        &rhs,
    )?)
}
