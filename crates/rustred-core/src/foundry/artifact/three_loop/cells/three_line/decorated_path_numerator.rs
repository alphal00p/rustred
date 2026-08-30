//! Generated recurrences for the decorated three-line path sector.
//!
//! The owned targets are `J(0,0,2,N,1,1)` with `N < 0`.  A complete
//! depth-one translated-source span discovers the recurrence.  The bulk is
//! then rebuilt from exactly those generated rows whose individual shifts
//! remain representable at the `i64` lower endpoint; no algebra is authored.
//! The resulting children with negative inactive powers remain explicit
//! numerator obligations rather than being mislabeled as factorization.

use crate::algebra::{IndexedAlgebraLimits, IndexedCoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::artifact::ArtifactError;
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
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

pub(super) const PATH_SECTOR: [i64; 6] = [0, 0, 1, 0, 1, 1];
pub(super) const PATH_NUMERATOR_PIVOT: [i64; 6] = [0, 0, 1, -1, 0, 0];
pub(super) const BULK_REPLAY_ANCHOR: [i64; 6] = [-8, -8, 8, -8, 8, 8];
pub(super) const FREE_POSITION: usize = 3;
const SEARCH_DEPTH: usize = 1;

pub(super) struct PathNumeratorSelectionWitness {
    pub(super) direct_endpoint_sources: SourceViewBatch,
    pub(super) direct_endpoint_rule: ParametricRule,
    pub(super) complete_free_sources: SourceViewBatch,
    pub(super) complete_free_rule: ParametricRule,
    pub(super) machine_safe_sources: SourceViewBatch,
    pub(super) machine_safe_rule: ParametricRule,
}

pub(super) struct PathNumeratorBuild {
    pub(super) context: IndexedCoefficientContext,
    pub(super) endpoint: RuleCell,
    pub(super) bulk: RuleCell,
    pub(super) direct_endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) endpoint_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) machine_safe_complete_source_ordinals: Box<[usize]>,
    pub(super) bulk_selected_complete_source_ordinals: Box<[usize]>,
    pub(super) selection_witness: Option<PathNumeratorSelectionWitness>,
}

pub(super) fn derive_decorated_path_numerator_cells()
-> Result<(IndexedCoefficientContext, RuleCell, RuleCell), ArtifactError> {
    let build = derive_path_numerator_build(false)?;
    Ok((build.context, build.endpoint, build.bulk))
}

pub(super) fn derive_path_numerator_build(
    retain_selection_witness: bool,
) -> Result<PathNumeratorBuild, ArtifactError> {
    let family = canonical_family()?;
    let canonicalizer = canonical_s4(&family)?;
    let zero_sectors = exact_zero_sectors(&canonicalizer)?;
    let generator =
        ParametricIbpGenerator::try_new_with_config(&family, ParametricIbpConfig::default())?;
    let (completed, _ordinary_source_count) = complete_ordinary_sources(&generator)?;
    let search = SectorSearchDiamond::try_new(
        IntegralKey::try_new(PATH_SECTOR)?,
        SEARCH_DEPTH,
        SectorSearchLimits::default(),
    )?;

    // Keep an independent exact-corner derivation as minimal-depth evidence.
    // Production uses the common free-face rule below so that its endpoint is
    // an exact coefficient specialization of the same generated identity.
    let direct_endpoint_sources = project_complete_endpoint_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let direct_endpoint_rule = derive_direct_endpoint_rule(&generator, &direct_endpoint_sources)?;
    let direct_endpoint_selected_complete_source_ordinals =
        selected_source_ordinals(&direct_endpoint_rule);

    // The complete free-face projection needs two cells of lower headroom
    // because four of its generated rows contain a -2 shift in n3.
    let complete_free_sources = project_complete_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &canonicalizer,
        &zero_sectors,
    )?;
    let complete_free_rule = derive_free_rule(&generator, &complete_free_sources)?;
    let endpoint_selected_complete_source_ordinals = selected_source_ordinals(&complete_free_rule);

    let endpoint_sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &endpoint_selected_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let endpoint_rule = derive_free_rule(&generator, &endpoint_sources)?;
    let endpoint_pruned = exactly_zero_rhs_ordinals(&generator, &endpoint_rule, 0)?;
    let endpoint_application = application_domain(&endpoint_rule, InteriorBounds::new(0, 0))?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        endpoint_rule,
        endpoint_sources,
        endpoint_application,
        fixed_endpoint(),
        endpoint_pruned,
        RuleCellLimits::default(),
    )?;

    // Select every complete generated relation whose concrete integral shifts
    // are representable over the full intended source box.  The parametric
    // solver then chooses the needed rows from that machine-safe span.
    let safe_domain = free_source_domain(i64::MIN + 1)?;
    let machine_safe_complete_source_ordinals = complete_free_sources
        .relations()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, relation)| {
            relation_is_machine_representable(relation, &safe_domain).then_some(ordinal)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let machine_safe_sources = project_selected_free_sources(
        &generator,
        &completed,
        search.offsets().iter().cloned(),
        &machine_safe_complete_source_ordinals,
        &canonicalizer,
        &zero_sectors,
    )?;
    let machine_safe_rule = derive_free_rule(&generator, &machine_safe_sources)?;
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
    let bulk_rule = derive_free_rule(&generator, &bulk_sources)?;
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

    let selection_witness = retain_selection_witness.then_some(PathNumeratorSelectionWitness {
        direct_endpoint_sources,
        direct_endpoint_rule,
        complete_free_sources,
        complete_free_rule,
        machine_safe_sources,
        machine_safe_rule,
    });
    let context = generator.context().clone();
    drop(generator);
    Ok(PathNumeratorBuild {
        context,
        endpoint,
        bulk,
        direct_endpoint_selected_complete_source_ordinals,
        endpoint_selected_complete_source_ordinals,
        machine_safe_complete_source_ordinals,
        bulk_selected_complete_source_ordinals,
        selection_witness,
    })
}

pub(super) fn derive_direct_endpoint_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &PATH_SECTOR,
        &PATH_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

pub(super) fn derive_free_rule(
    generator: &ParametricIbpGenerator<'_>,
    sources: &SourceViewBatch,
) -> Result<ParametricRule, ArtifactError> {
    Ok(derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &BULK_REPLAY_ANCHOR,
        &PATH_NUMERATOR_PIVOT,
        OrderingPolicy::default(),
        ParametricRuleLimits::default(),
    )?)
}

pub(super) fn project_complete_endpoint_sources(
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

pub(super) fn project_complete_free_sources(
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

fn selected_source_ordinals(rule: &ParametricRule) -> Box<[usize]> {
    rule.source_combination()
        .iter()
        .map(|contribution| contribution.source_ordinal())
        .collect::<Vec<_>>()
        .into_boxed_slice()
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

fn exactly_zero_rhs_ordinals(
    generator: &ParametricIbpGenerator<'_>,
    rule: &ParametricRule,
    free_value: i64,
) -> Result<Vec<usize>, ArtifactError> {
    rule.right_hand_side()
        .iter()
        .enumerate()
        .filter_map(|(ordinal, term)| {
            let result = generator.context().specialize_fixed_indices_sealed(
                term.coefficient(),
                &[(FREE_POSITION, free_value)],
                IndexedAlgebraLimits::default(),
            );
            match result {
                Ok((coefficient, _guard)) => coefficient.is_zero().then_some(Ok(ordinal)),
                Err(error) => Some(Err(ArtifactError::from(error))),
            }
        })
        .collect()
}

pub(super) const fn path_numerator_search_depth() -> usize {
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
    std::array::from_fn(|position| FixedIndexRestriction::new(position, PATH_SECTOR[position]))
}

fn endpoint_source_domain() -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&PATH_SECTOR)?,
        PATH_SECTOR.map(|value| InteriorBounds::new(value, value)),
    )?)
}

fn free_source_domain(lower: i64) -> Result<SectorInteriorDomain, ArtifactError> {
    Ok(SectorInteriorDomain::try_new(
        Mask::try_from_indices(&PATH_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(lower, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
    )?)
}

fn application_domain(
    rule: &ParametricRule,
    free_bounds: InteriorBounds,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&PATH_SECTOR)?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            free_bounds,
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        rule.pivot().values(),
        &rhs,
    )?)
}
