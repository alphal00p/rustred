use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily, IntegralFamilyLimits, IntegralKey};
use crate::foundry::cell::{FixedIndexRestriction, RuleCell, RuleCellLimits, SourceViewBatch};
use crate::foundry::parametric::{
    ParametricRule, ParametricRuleLimits, derive_sector_interior_rule_for_target,
    derive_sector_monotone_rule_for_target,
};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
    TranslatedSourceLimits,
};
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{
    CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits as SymmetryLimits,
    MomentumMap, verify,
};
use crate::sector::{
    InteriorBounds, Mask, OrderingPolicy, SectorInteriorDomain, SectorMonotoneDomain,
};

use super::error::ArtifactError;
use super::factorization::{FactorizationFactor, FactorizationRule, UnimodularLoopBasis};
use super::install::{ClosingArtifactCandidate, install};
use super::model::{
    ArtifactSchemaVersion, ClosedArtifact, CommonMassHomogeneityProof, ZeroSectorTerminal,
    ZeroTerminalProof,
};
use super::one_loop::derive_one_loop_unit_mass_tadpole_with_limits;

pub(super) const ALGORITHM_ID: &str = "rustred.generated.two-loop-unit-mass-sunset.v1";

const TOP_SOURCE_ORDINALS: &[usize] = &[0, 1, 2, 3];
const PAIR_SOURCE_ORDINALS: &[usize] = &[0];
const CORNER_SOURCE_ORDINALS: &[usize] = &[0, 2, 4, 6];

/// Generate and seal the equal-mass two-loop vacuum sunset family over
/// `Q(d)`, including every numerator power and every pinched subsector.
///
/// No recurrence is authored. Five disjoint application cells are obtained
/// from four ordinary source rows: one top-sector cell, bulk/boundary cells
/// on the canonical two-line sector, and bulk/endpoint cells on its fixed
/// active corner. The remaining two-line face factorizes into two immutable
/// one-loop dependencies. Exact `S3` routing makes this single family own all
/// three equivalent pinches.
pub fn derive_two_loop_unit_mass_sunset() -> Result<ClosedArtifact, ArtifactError> {
    derive_two_loop_unit_mass_sunset_with_limits(
        IntegralFamilyLimits::default(),
        ParametricIbpConfig::default(),
        ParametricRuleLimits::default(),
    )
}

pub(super) fn derive_two_loop_unit_mass_sunset_with_limits(
    family_limits: IntegralFamilyLimits,
    source_generation: ParametricIbpConfig,
    rule_derivation: ParametricRuleLimits,
) -> Result<ClosedArtifact, ArtifactError> {
    let family = canonical_family(family_limits)?;
    let generator = ParametricIbpGenerator::try_new_with_config(&family, source_generation)?;
    let completed = complete_ordinary_sources(&generator)?;
    let canonicalizer = canonical_s3(&family)?;
    let zero_masks = zero_sector_masks()?;

    let top = build_top_cell(&generator, &completed, rule_derivation)?;
    let (pair_bulk, pair_boundary) = build_pair_cells(&generator, &completed, rule_derivation)?;
    let (corner_bulk, corner_endpoint) = build_corner_cells(
        &generator,
        &completed,
        &canonicalizer,
        &zero_masks,
        rule_derivation,
    )?;

    let context = generator.context().clone();
    drop(generator);

    let sunset_master = IntegralKey::try_new([1, 1, 1])?;
    let product_master = IntegralKey::try_new([0, 1, 1])?;
    let mut masters = BTreeSet::new();
    masters.insert(sunset_master);
    masters.insert(product_master.clone());

    let dependency = derive_one_loop_unit_mass_tadpole_with_limits(
        family_limits,
        source_generation,
        rule_derivation,
    )?;
    let product_domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&[0, 1, 1])?,
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX),
        ],
    )?;
    let factorization = FactorizationRule::new(
        product_domain,
        [
            FactorizationFactor::new(0, [1], [0]),
            FactorizationFactor::new(0, [2], [1]),
        ],
        family.coefficient_context().one(),
        // q0 = k2, q1 = k1+k2. The determinant is -1, D1=q0^2-1,
        // and D2=q1^2-1, so the pinched measure and denominators split
        // exactly into two one-loop tadpoles.
        UnimodularLoopBasis::new(2, [0, 1, 1, 1]),
    );

    let zero_sectors = zero_masks
        .into_iter()
        .map(|sector| {
            let proof = if sector.active_bits().iter().any(|&active| active) {
                ZeroTerminalProof::LeePomeranskyRankDeficiency
            } else {
                ZeroTerminalProof::ScalelessVacuumPolynomial
            };
            ZeroSectorTerminal::new(sector, proof)
        })
        .collect();

    install(ClosingArtifactCandidate {
        schema: ArtifactSchemaVersion::CURRENT,
        algorithm_id: ALGORITHM_ID,
        arity: 3,
        ordering: OrderingPolicy::default(),
        // The recurrence is exact over the complete mathematical integer
        // lattice. Its fixed i64 representation needs one unit of headroom
        // at the public root because a top-cell child can transiently reach
        // i64::MAX before descending. This is a representation boundary, not
        // a physical or algorithmic rank restriction.
        supported_root_power_bounds: vec![
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
            InteriorBounds::new(i64::MIN, i64::MAX - 1),
        ]
        .into_boxed_slice(),
        family,
        context,
        source_relations: completed.into_relations(),
        rules: Vec::new(),
        rule_cells: vec![
            Arc::new(top),
            Arc::new(pair_bulk),
            Arc::new(pair_boundary),
            Arc::new(corner_bulk),
            Arc::new(corner_endpoint),
        ],
        canonicalizer: Some(canonicalizer),
        dependencies: vec![Box::new(dependency)],
        factorization_rules: vec![factorization],
        masters,
        zero_sectors,
        common_mass_homogeneity: Some(CommonMassHomogeneityProof::UniformVacuumMassSquared),
    })
}

fn canonical_family(limits: IntegralFamilyLimits) -> Result<IntegralFamily, ArtifactError> {
    let base = CoefficientContext::try_new(["d"])?;
    let dimension = base
        .parameter("d")
        .expect("the authenticated coefficient context contains d");
    let zero = base.zero();
    let one = base.one();
    let two = base.integer(2);
    let minus_one = base.integer(-1);
    Ok(IntegralFamily::new_with_limits(
        "rustred-two-loop-unit-mass-sunset-v1",
        vec!["k1".to_owned(), "k2".to_owned()],
        Vec::new(),
        base,
        dimension,
        vec![
            AffineDenominator::new(
                minus_one.clone(),
                vec![one.clone(), zero.clone(), zero.clone()],
            ),
            AffineDenominator::new(
                minus_one.clone(),
                vec![zero.clone(), zero.clone(), one.clone()],
            ),
            AffineDenominator::new(minus_one, vec![one.clone(), two, one]),
        ],
        Vec::new(),
        vec![zero.clone(), zero.clone(), zero],
        limits,
    )?)
}

fn complete_ordinary_sources(
    generator: &ParametricIbpGenerator<'_>,
) -> Result<CompletedIbpSourceRows, ArtifactError> {
    let prepared = generator.prepare_ordinary_ibp()?;
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    Ok(prepared.complete(rows)?)
}

fn canonical_s3(family: &IntegralFamily) -> Result<Canonicalizer, ArtifactError> {
    let coefficients = family.coefficient_context();
    let generators = [
        // k1 <-> k2, hence D0 <-> D1.
        vacuum_map(coefficients, [0, 1, 1, 0])?,
        // k1 -> k1, k2 -> -k1-k2, hence D1 <-> D2.
        vacuum_map(coefficients, [1, 0, -1, -1])?,
    ]
    .into_iter()
    .map(|map| {
        let verified = verify(family, family, map, SymmetryLimits::default())?;
        Ok(compile(family, verified)?)
    })
    .collect::<Result<Vec<_>, ArtifactError>>()?;
    Ok(Canonicalizer::try_new(
        OrderingPolicy::default(),
        generators,
        CanonicalizationLimits::default(),
    )?)
}

fn vacuum_map(
    coefficients: &CoefficientContext,
    entries: [i64; 4],
) -> Result<MomentumMap, ArtifactError> {
    Ok(MomentumMap::new(
        CoefficientMatrix::try_new(
            2,
            2,
            entries.into_iter().map(|entry| coefficients.integer(entry)),
        )?,
        CoefficientMatrix::try_new(2, 0, [])?,
        CoefficientMatrix::try_new(0, 0, [])?,
    ))
}

fn zero_sector_masks() -> Result<Vec<Mask>, ArtifactError> {
    [
        &[0, 0, 0][..],
        &[0, 0, 1][..],
        &[0, 1, 0][..],
        &[1, 0, 0][..],
    ]
    .into_iter()
    .map(Mask::try_from_indices)
    .collect::<Result<Vec<_>, _>>()
    .map_err(ArtifactError::from)
}

fn build_top_cell(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    rule_derivation: ParametricRuleLimits,
) -> Result<RuleCell, ArtifactError> {
    let sources = direct_sources(generator, completed, [0, 0, 0], TOP_SOURCE_ORDINALS)?;
    let rule = derive_sector_monotone_rule_for_target(
        generator.context(),
        sources.relations(),
        &[1, 1, 1],
        &[0, 0, 1],
        OrderingPolicy::default(),
        rule_derivation,
    )?;
    require_rule_layout(
        &rule,
        &[0, 0, 1],
        &[
            &[0, 1, -1],
            &[0, 0, 0],
            &[0, -1, 1],
            &[-1, 1, 0],
            &[-1, 0, 1],
        ],
    )?;
    let application = application_domain(
        [1, 1, 1],
        [
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(1, i64::MAX - 1),
            InteriorBounds::new(1, i64::MAX - 1),
        ],
        &rule,
    )?;
    Ok(RuleCell::try_refined(
        generator.context(),
        rule,
        sources,
        application,
        [],
        [],
        RuleCellLimits::default(),
    )?)
}

fn build_pair_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    rule_derivation: ParametricRuleLimits,
) -> Result<(RuleCell, RuleCell), ArtifactError> {
    let sources = direct_sources(generator, completed, [0, 0, -1], PAIR_SOURCE_ORDINALS)?;
    let rule = derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &[-2, 2, 2],
        &[-1, 0, 0],
        OrderingPolicy::default(),
        rule_derivation,
    )?;
    require_rule_layout(
        &rule,
        &[-1, 0, 0],
        &[&[0, 0, 0], &[0, 0, -1], &[0, -1, 0], &[1, 0, -1]],
    )?;
    let bulk_domain = application_domain(
        [0, 1, 1],
        [
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
        ],
        &rule,
    )?;
    let bulk = RuleCell::try_refined(
        generator.context(),
        rule.clone(),
        sources,
        bulk_domain,
        [],
        [],
        RuleCellLimits::default(),
    )?;

    let boundary_sources = direct_sources(generator, completed, [0, 0, -1], PAIR_SOURCE_ORDINALS)?;
    let boundary_domain = application_domain(
        [0, 1, 1],
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, i64::MAX),
            InteriorBounds::new(2, i64::MAX),
        ],
        &rule,
    )?;
    let boundary = RuleCell::try_refined(
        generator.context(),
        rule,
        boundary_sources,
        boundary_domain,
        [FixedIndexRestriction::new(0, 0)],
        [3],
        RuleCellLimits::default(),
    )?;
    Ok((bulk, boundary))
}

fn build_corner_cells(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &Canonicalizer,
    zero_sectors: &[Mask],
    rule_derivation: ParametricRuleLimits,
) -> Result<(RuleCell, RuleCell), ArtifactError> {
    let sources = projected_corner_sources(generator, completed, canonicalizer, zero_sectors)?;
    let rule = derive_sector_interior_rule_for_target(
        generator.context(),
        sources.relations(),
        &[-1, 1, 1],
        &[-1, 0, 0],
        OrderingPolicy::default(),
        rule_derivation,
    )?;
    require_rule_layout(&rule, &[-1, 0, 0], &[&[0, 0, 0], &[1, 0, 0]])?;
    let bulk_domain = application_domain(
        [0, 1, 1],
        [
            InteriorBounds::new(i64::MIN + 1, -1),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        &rule,
    )?;
    let fixed_active = [
        FixedIndexRestriction::new(1, 1),
        FixedIndexRestriction::new(2, 1),
    ];
    let bulk = RuleCell::try_refined(
        generator.context(),
        rule.clone(),
        sources,
        bulk_domain,
        fixed_active,
        [],
        RuleCellLimits::default(),
    )?;

    let endpoint_sources =
        projected_corner_sources(generator, completed, canonicalizer, zero_sectors)?;
    let endpoint_domain = application_domain(
        [0, 1, 1],
        [
            InteriorBounds::new(0, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
        &rule,
    )?;
    let endpoint = RuleCell::try_refined(
        generator.context(),
        rule,
        endpoint_sources,
        endpoint_domain,
        [
            FixedIndexRestriction::new(0, 0),
            FixedIndexRestriction::new(1, 1),
            FixedIndexRestriction::new(2, 1),
        ],
        [1],
        RuleCellLimits::default(),
    )?;
    Ok((bulk, endpoint))
}

fn direct_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    offset: [i64; 3],
    ordinals: &[usize],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        [IntegralShift::try_new(offset)?],
        TranslatedSourceLimits::default(),
    )?;
    Ok(SourceViewBatch::try_select(
        translated,
        ordinals,
        RuleCellLimits::default(),
    )?)
}

fn projected_corner_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
    canonicalizer: &Canonicalizer,
    zero_sectors: &[Mask],
) -> Result<SourceViewBatch, ArtifactError> {
    let translated = generator.translate_completed_source_rows(
        completed,
        [
            IntegralShift::try_new([-1, 0, 0])?,
            IntegralShift::try_new([0, 0, 0])?,
        ],
        TranslatedSourceLimits::default(),
    )?;
    let domain = SectorInteriorDomain::try_new(
        Mask::try_from_indices(&[0, 1, 1])?,
        [
            InteriorBounds::new(i64::MIN + 1, 0),
            InteriorBounds::new(1, 1),
            InteriorBounds::new(1, 1),
        ],
    )?;
    Ok(SourceViewBatch::try_project_residual(
        translated,
        CORNER_SOURCE_ORDINALS,
        generator.context(),
        domain,
        [
            FixedIndexRestriction::new(1, 1),
            FixedIndexRestriction::new(2, 1),
        ],
        canonicalizer,
        zero_sectors,
        RuleCellLimits::default(),
    )?)
}

fn application_domain(
    sector: [i64; 3],
    bounds: [InteriorBounds; 3],
    rule: &ParametricRule,
) -> Result<SectorMonotoneDomain, ArtifactError> {
    let rhs = rule
        .right_hand_side()
        .iter()
        .map(|term| term.shift().values())
        .collect::<Vec<_>>();
    Ok(SectorMonotoneDomain::try_new_for_rule(
        Mask::try_from_indices(&sector)?,
        bounds,
        rule.pivot().values(),
        &rhs,
    )?)
}

fn require_rule_layout(
    rule: &ParametricRule,
    pivot: &[i64],
    rhs: &[&[i64]],
) -> Result<(), ArtifactError> {
    if rule.pivot().values() != pivot
        || rule.right_hand_side().len() != rhs.len()
        || rule
            .right_hand_side()
            .iter()
            .zip(rhs)
            .any(|(term, expected)| term.shift().values() != *expected)
    {
        return Err(ArtifactError::InvalidRuleShape {
            detail: "the generated sunset rule has an unexpected exact shift layout",
        });
    }
    Ok(())
}
