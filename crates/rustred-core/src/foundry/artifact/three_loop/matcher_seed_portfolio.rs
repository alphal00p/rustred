//! Non-authoritative Vakint-root seed and local-chart experiment.
//!
//! Each input is compiled structurally from active parent slots. The frozen
//! matcher routing is retained only as a shape-checked, unimodular candidate:
//! this fixture does not authenticate it as a simultaneous routing witness or
//! apply it to the denominators. The ISP completion therefore remains in the
//! parent loop coordinates. Every resulting family is a cold search chart
//! only. It cannot replace the parent K6 family fingerprint, its complete
//! contraction plan, or parent-source replay.

use std::collections::BTreeSet;

use crate::algebra::Coefficient;
use crate::family::isp::IspCompletion;
use crate::family::{IntegralFamily, IntegralKey, invert_symbolic_matrix};
use crate::foundry::completion::source_discovery::{
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits,
};
use crate::foundry::completion::{CompletePhysicalContractionGoal, FamilyCoverageLimits};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
};

use super::manifest::{FULL_RANK_ORBITS, VAKINT_CLASSES, VakintClassWitness, ZERO_ORBITS};
use super::tests::canonical_presentation;
use super::{canonical_family, canonical_s4};

const ARITY: usize = 6;
const LOOP_COUNT: usize = 3;
const MATCHER_SEED_COUNT: usize = 5;
const COMPLETE_RAW_SECTOR_COUNT: usize = 64;
const SEEDED_RAW_FULL_RANK_SECTOR_COUNT: usize = 34;
const MISSING_STAR: [i64; ARITY] = [0, 0, 1, 1, 0, 1];

#[derive(Debug)]
struct MatcherSeedChart {
    /// Diagnostics only: no decision below depends on this name.
    diagnostic_label: &'static str,
    raw_corner: IntegralKey,
    canonical_corner: IntegralKey,
    loop_basis_candidate: Box<[i64]>,
    loop_basis_candidate_determinant: Coefficient,
    raw_orbit_size: usize,
    completion: IspCompletion,
}

#[derive(Debug)]
struct MatcherSeedPortfolio {
    parent_family_fingerprint: String,
    complete_raw_sector_count: usize,
    seeded_raw_sector_count: usize,
    charts: Box<[MatcherSeedChart]>,
    unseeded_required_orbits: Box<[(IntegralKey, usize)]>,
}

impl MatcherSeedPortfolio {
    fn try_compile() -> Result<Self, Box<dyn std::error::Error>> {
        let presentation = canonical_presentation(&[]);
        let parent = presentation.family();
        let canonicalizer = canonical_s4(parent)?;
        let goal = CompletePhysicalContractionGoal::try_new(&presentation)?;
        let plan = goal.try_plan(&canonicalizer, FamilyCoverageLimits::default())?;

        let mut charts = Vec::new();
        charts.try_reserve_exact(VAKINT_CLASSES.len())?;
        for witness in VAKINT_CLASSES {
            let raw_corner = IntegralKey::try_new(witness.active_slots.into_iter().map(i64::from))?;
            let canonical_corner = canonicalizer.canonicalize(&raw_corner)?.canonical().clone();
            if canonical_corner.powers() != witness.canonical_sector {
                return Err(format!(
                    "{} canonicalized outside its frozen matcher witness",
                    witness.label
                )
                .into());
            }
            let required = plan
                .required_orbits()
                .iter()
                .find(|orbit| orbit.corner() == &canonical_corner)
                .ok_or("a matcher seed is outside the complete K6 contraction plan")?;

            let loop_basis_candidate = checked_loop_basis_candidate(parent, witness)?;
            let matrix = loop_basis_candidate
                .chunks_exact(LOOP_COUNT)
                .map(|row| {
                    row.iter()
                        .map(|&entry| parent.coefficient_context().integer(entry))
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let (_, determinant) = invert_symbolic_matrix(
                parent.coefficient_context(),
                &matrix,
                parent.construction_limits(),
            )?;
            if determinant != parent.coefficient_context().one()
                && determinant != parent.coefficient_context().integer(-1)
            {
                return Err("a matcher seed supplied a non-unimodular loop basis".into());
            }

            charts.push(MatcherSeedChart {
                diagnostic_label: witness.label,
                raw_corner,
                canonical_corner,
                loop_basis_candidate,
                loop_basis_candidate_determinant: determinant,
                raw_orbit_size: required.raw_sector_count(),
                completion: complete_seed_chart(parent, witness)?,
            });
        }
        charts.sort_unstable_by(|left, right| {
            left.canonical_corner
                .cmp(&right.canonical_corner)
                .then_with(|| left.raw_corner.cmp(&right.raw_corner))
                .then_with(|| left.loop_basis_candidate.cmp(&right.loop_basis_candidate))
        });
        if charts.windows(2).any(|pair| {
            pair[0].canonical_corner == pair[1].canonical_corner
                && pair[0].raw_corner == pair[1].raw_corner
                && pair[0].loop_basis_candidate == pair[1].loop_basis_candidate
        }) {
            return Err("the matcher seed portfolio repeats one exact chart input".into());
        }

        let seeded = charts
            .iter()
            .map(|chart| chart.canonical_corner.clone())
            .collect::<BTreeSet<_>>();
        let seeded_raw_sector_count = plan
            .required_orbits()
            .iter()
            .filter(|orbit| seeded.contains(orbit.corner()))
            .map(|orbit| orbit.raw_sector_count())
            .sum();
        let unseeded_required_orbits = plan
            .required_orbits()
            .iter()
            .filter(|orbit| !seeded.contains(orbit.corner()))
            .map(|orbit| (orbit.corner().clone(), orbit.raw_sector_count()))
            .collect::<Vec<_>>()
            .into_boxed_slice();

        Ok(Self {
            parent_family_fingerprint: parent.fingerprint().to_owned(),
            complete_raw_sector_count: plan.raw_sector_count(),
            seeded_raw_sector_count,
            charts: charts.into_boxed_slice(),
            unseeded_required_orbits,
        })
    }
}

fn checked_loop_basis_candidate(
    family: &IntegralFamily,
    witness: VakintClassWitness,
) -> Result<Box<[i64]>, Box<dyn std::error::Error>> {
    let expected = family
        .loop_count()
        .checked_mul(family.loop_count())
        .ok_or("matcher seed loop-basis size overflow")?;
    if witness.routing_rows.len() != expected {
        return Err("matcher seed loop basis has the wrong shape".into());
    }
    Ok(witness.routing_rows.into())
}

fn complete_seed_chart(
    parent: &IntegralFamily,
    witness: VakintClassWitness,
) -> Result<IspCompletion, Box<dyn std::error::Error>> {
    let active_slots = witness
        .active_slots
        .iter()
        .enumerate()
        .filter_map(|(slot, &active)| active.then_some(slot))
        .collect::<Vec<_>>();
    let chart_id = witness
        .active_slots
        .iter()
        .map(|&active| if active { '1' } else { '0' })
        .collect::<String>();
    Ok(IspCompletion::try_new(
        format!("rustred-k6-sector-seed-chart-{chart_id}"),
        parent.loop_momenta().to_vec(),
        parent.external_momenta().to_vec(),
        parent.coefficient_context().clone(),
        parent.dimension().clone(),
        active_slots
            .iter()
            .map(|&slot| parent.denominators()[slot].clone())
            .collect(),
        parent.external_gram().to_vec(),
        active_slots
            .iter()
            .map(|&slot| parent.power_shifts()[slot].clone())
            .collect(),
    )?)
}

fn complete_ordinary(
    generator: &ParametricIbpGenerator<'_>,
) -> Result<CompletedIbpSourceRows, Box<dyn std::error::Error>> {
    let prepared = generator.prepare_ordinary_ibp()?;
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    Ok(prepared.complete(rows)?)
}

#[test]
fn all_vakint_roots_compile_as_foreign_search_charts_inside_one_k6_plan() {
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    assert_eq!(portfolio.charts.len(), MATCHER_SEED_COUNT);
    assert_eq!(
        portfolio.complete_raw_sector_count,
        COMPLETE_RAW_SECTOR_COUNT
    );
    assert_eq!(
        portfolio.seeded_raw_sector_count,
        SEEDED_RAW_FULL_RANK_SECTOR_COUNT
    );
    assert_eq!(
        portfolio.complete_raw_sector_count - portfolio.seeded_raw_sector_count,
        30
    );

    let family = canonical_family().unwrap();
    assert_eq!(portfolio.parent_family_fingerprint, family.fingerprint());
    let context = family.coefficient_context();
    for chart in &portfolio.charts {
        assert_eq!(chart.raw_corner.powers().len(), ARITY);
        assert_eq!(chart.canonical_corner.powers().len(), ARITY);
        assert_eq!(chart.loop_basis_candidate.len(), LOOP_COUNT * LOOP_COUNT);
        assert!(
            chart.loop_basis_candidate_determinant == context.one()
                || chart.loop_basis_candidate_determinant == context.integer(-1),
            "{} lost its unimodular routing candidate",
            chart.diagnostic_label
        );
        assert_eq!(chart.completion.family().denominator_count(), ARITY);
        assert_eq!(
            chart.completion.input_denominator_count()
                + chart.completion.appended_coordinate_ordinals().len(),
            ARITY
        );
        assert_ne!(
            chart.completion.family().fingerprint(),
            portfolio.parent_family_fingerprint,
            "a local chart must not inherit parent K6 family authority"
        );
    }

    let seeded = portfolio
        .charts
        .iter()
        .map(|chart| chart.canonical_corner.powers().to_vec())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        seeded,
        VAKINT_CLASSES
            .iter()
            .map(|witness| witness.canonical_sector.to_vec())
            .collect()
    );
    assert_eq!(
        portfolio
            .charts
            .iter()
            .map(|chart| chart.raw_orbit_size)
            .sum::<usize>(),
        SEEDED_RAW_FULL_RANK_SECTOR_COUNT
    );

    let unseeded = portfolio
        .unseeded_required_orbits
        .iter()
        .map(|(corner, size)| (corner.powers().to_vec(), *size))
        .collect::<BTreeSet<_>>();
    let expected_unseeded = ZERO_ORBITS
        .iter()
        .map(|orbit| (orbit.representative.to_vec(), orbit.size))
        .chain(std::iter::once((MISSING_STAR.to_vec(), 4)))
        .collect::<BTreeSet<_>>();
    assert_eq!(unseeded, expected_unseeded);

    let missing_full_rank = FULL_RANK_ORBITS
        .iter()
        .filter(|orbit| !seeded.contains(orbit.representative.as_slice()))
        .map(|orbit| orbit.representative)
        .collect::<Vec<_>>();
    assert_eq!(missing_full_rank, [MISSING_STAR]);
}

#[test]
fn local_seed_completion_is_deterministic_and_the_s5_isp_is_s23() {
    let first = MatcherSeedPortfolio::try_compile().unwrap();
    let second = MatcherSeedPortfolio::try_compile().unwrap();
    let census = |portfolio: &MatcherSeedPortfolio| {
        portfolio
            .charts
            .iter()
            .map(|chart| {
                (
                    chart.raw_corner.powers().to_vec(),
                    chart.canonical_corner.powers().to_vec(),
                    chart.completion.input_denominator_count(),
                    chart.completion.appended_coordinate_ordinals().to_vec(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(census(&first), census(&second));

    let s5 = first
        .charts
        .iter()
        .find(|chart| chart.raw_corner.powers() == [1, 1, 1, 1, 1, 0])
        .unwrap();
    assert_eq!(s5.completion.input_denominator_count(), 5);
    assert_eq!(s5.completion.appended_coordinate_ordinals(), [4]);
    let isp = &s5.completion.family().denominators()[5];
    let context = s5.completion.family().coefficient_context();
    assert_eq!(isp.constant(), &context.zero());
    assert_eq!(
        isp.coefficients(),
        [
            context.zero(),
            context.zero(),
            context.zero(),
            context.zero(),
            context.one(),
            context.zero(),
        ]
    );

    // In the parent family this unit row obeys
    // 2*s23 = 1 + D2 + D3 - D6. Replay every affine component exactly.
    let parent = canonical_family().unwrap();
    let parent_context = parent.coefficient_context();
    let limits = parent.construction_limits().exact_algebra;
    let d2 = &parent.denominators()[1];
    let d3 = &parent.denominators()[2];
    let d6 = &parent.denominators()[5];
    for (component, isp_component) in std::iter::once(isp.constant())
        .chain(isp.coefficients())
        .enumerate()
    {
        let left = parent_context
            .try_mul(&parent_context.integer(2), isp_component, limits)
            .unwrap();
        let constant = if component == 0 {
            parent_context.one()
        } else {
            parent_context.zero()
        };
        let right = parent_context
            .try_add(&constant, affine_component(d2, component), limits)
            .and_then(|value| {
                parent_context.try_add(&value, affine_component(d3, component), limits)
            })
            .and_then(|value| {
                parent_context.try_sub(&value, affine_component(d6, component), limits)
            })
            .unwrap();
        assert_eq!(
            left, right,
            "S5 affine replay failed at component {component}"
        );
    }
}

fn affine_component(
    denominator: &crate::family::AffineDenominator,
    component: usize,
) -> &Coefficient {
    if component == 0 {
        denominator.constant()
    } else {
        &denominator.coefficients()[component - 1]
    }
}

#[test]
fn matcher_root_seeding_does_not_install_foreign_rows_into_parent_authority() {
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let parent = canonical_family().unwrap();
    let parent_generator =
        ParametricIbpGenerator::try_new_with_config(&parent, ParametricIbpConfig::default())
            .unwrap();
    let parent_completed = complete_ordinary(&parent_generator).unwrap();
    assert_eq!(parent_completed.source_row_count(), 9);
    let zero = IntegralShift::try_new([0; ARITY]).unwrap();
    let limits = SourceDiscoveryLimits::default();
    let zero_sources = parent_generator
        .translate_completed_source_rows(&parent_completed, [zero.clone()], limits.translation)
        .unwrap();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&zero_sources, limits).unwrap();
    let baseline = incidence.try_nominate_target_unit(&zero, limits).unwrap();
    assert_eq!(baseline.requests().len(), 90);

    // Merely enumerating seed charts cannot mutate the immutable parent
    // incidence index. This is an authority-separation check, not a claim that
    // a transformed local IBP module has the same span as the parent module.
    for _chart in &portfolio.charts {
        assert_eq!(
            incidence.try_nominate_target_unit(&zero, limits).unwrap(),
            baseline
        );
    }

    for chart in &portfolio.charts {
        let local = chart.completion.family();
        let local_generator =
            ParametricIbpGenerator::try_new_with_config(local, ParametricIbpConfig::default())
                .unwrap();
        let local_completed = complete_ordinary(&local_generator).unwrap();
        assert_eq!(local_completed.source_row_count(), 9);
        let local_sources = local_generator
            .translate_completed_source_rows(&local_completed, [zero.clone()], limits.translation)
            .unwrap();
        assert_eq!(local_sources.family_fingerprint(), local.fingerprint());
        assert_ne!(local_sources.family_fingerprint(), parent.fingerprint());
    }
}
