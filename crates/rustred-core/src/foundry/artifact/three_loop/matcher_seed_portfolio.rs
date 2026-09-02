//! Non-authoritative Vakint-root seed and local-chart experiment.
//!
//! Each input is compiled structurally from active parent slots. The frozen
//! matcher routing is applied exactly as `q = T k`: Symbolica inverts `T`,
//! routes the physical quadratics, and replays every ISP-completed local row
//! in the parent denominator basis. Every resulting family is nevertheless a
//! cold foreign search chart only. It cannot replace the parent K6 family
//! fingerprint, its complete contraction plan, or parent-source replay.

#[cfg(test)]
mod operator_transport_certificate;
mod routing;
mod support_only_parent_replay_falsifier;
mod transport;

use std::collections::BTreeSet;

use crate::algebra::Coefficient;
use crate::family::isp::IspCompletion;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::completion::frame::{OneSidedChartFrame, PhysicalFrameLimits};
use crate::foundry::completion::source_discovery::{
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits,
};
use crate::foundry::completion::{CompletePhysicalContractionGoal, FamilyCoverageLimits};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpConfig, ParametricIbpGenerator,
};
use crate::sector::Mask;

use super::manifest::{FULL_RANK_ORBITS, VAKINT_CLASSES, ZERO_ORBITS};
use super::tests::canonical_presentation;
use super::{canonical_family, canonical_s4, derive_k6_terminal_authority};
use routing::{
    ExactMatcherChartRouting, MatcherChartTransportError, MatcherChartTransportLimits,
    try_route_and_complete,
};

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
    parent_family_fingerprint: String,
    raw_corner: IntegralKey,
    canonical_corner: IntegralKey,
    routing: ExactMatcherChartRouting,
    raw_orbit_size: usize,
    completion: IspCompletion,
    /// The complete nine-row ordinary source barrier is generated exactly
    /// once when the foreign chart is compiled. Fixed-sample experiments may
    /// select from this immutable chronology, but it never acquires parent
    /// K6 authority.
    ordinary: CompletedIbpSourceRows,
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

            let (completion, routing) = try_route_and_complete(parent, witness)?;
            let ordinary = {
                let generator = ParametricIbpGenerator::try_new_with_config(
                    completion.family(),
                    ParametricIbpConfig::default(),
                )?;
                complete_ordinary(&generator)?
            };

            charts.push(MatcherSeedChart {
                diagnostic_label: witness.label,
                parent_family_fingerprint: parent.fingerprint().to_owned(),
                raw_corner,
                canonical_corner,
                routing,
                raw_orbit_size: required.raw_sector_count(),
                completion,
                ordinary,
            });
        }
        charts.sort_unstable_by(|left, right| {
            left.canonical_corner
                .cmp(&right.canonical_corner)
                .then_with(|| left.raw_corner.cmp(&right.raw_corner))
                .then_with(|| left.routing.loop_basis().cmp(right.routing.loop_basis()))
        });
        if charts.windows(2).any(|pair| {
            pair[0].canonical_corner == pair[1].canonical_corner
                && pair[0].raw_corner == pair[1].raw_corner
                && pair[0].routing.loop_basis() == pair[1].routing.loop_basis()
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
        assert_eq!(chart.routing.loop_basis().len(), LOOP_COUNT * LOOP_COUNT);
        assert_eq!(
            chart.routing.inverse_loop_basis().len(),
            LOOP_COUNT * LOOP_COUNT
        );
        assert!(
            chart.routing.determinant() == &context.one()
                || chart.routing.determinant() == &context.integer(-1),
            "{} lost its exact unimodular routing",
            chart.diagnostic_label
        );
        assert_eq!(
            chart.routing.physical_parent_slots().len(),
            chart.completion.input_denominator_count()
        );
        assert_eq!(
            chart.routing.local_to_parent().len(),
            chart.completion.family().denominator_count()
        );
        assert_eq!(chart.completion.family().denominator_count(), ARITY);
        assert_eq!(
            chart.completion.input_denominator_count()
                + chart.completion.appended_coordinate_ordinals().len(),
            ARITY
        );
        assert!(chart.ordinary.is_complete_ordinary());
        assert_eq!(chart.ordinary.source_row_count(), LOOP_COUNT * LOOP_COUNT);
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

    let half = context
        .try_div(
            &context.one(),
            &context.integer(2),
            s5.completion.family().construction_limits().exact_algebra,
        )
        .unwrap();
    let minus_half = context
        .try_neg(
            &half,
            s5.completion.family().construction_limits().exact_algebra,
        )
        .unwrap();
    let parent_relation = &s5.routing.local_to_parent()[5];
    assert_eq!(parent_relation.constant(), &half);
    assert_eq!(
        parent_relation.denominator_coefficients(),
        [
            context.zero(),
            half.clone(),
            half,
            context.zero(),
            context.zero(),
            minus_half,
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

#[test]
fn a_nontrivial_matcher_route_freezes_both_auxiliary_parent_relations() {
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = portfolio
        .charts
        .iter()
        .find(|chart| chart.diagnostic_label == "I3L_pinch_1_6")
        .unwrap();
    assert_eq!(chart.routing.loop_basis(), [0, 1, 0, 0, 0, 1, -1, 0, 1]);
    assert_eq!(chart.completion.input_denominator_count(), 4);
    assert_eq!(chart.completion.appended_coordinate_ordinals(), [1, 2]);

    let context = chart.completion.family().coefficient_context();
    let half = context
        .try_div(
            &context.one(),
            &context.integer(2),
            chart
                .completion
                .family()
                .construction_limits()
                .exact_algebra,
        )
        .unwrap();
    let minus_half = context
        .try_neg(
            &half,
            chart
                .completion
                .family()
                .construction_limits()
                .exact_algebra,
        )
        .unwrap();

    // q1=k2, q2=k3, q3=-k1+k3. The appended coordinates therefore obey
    // 2 q1.q2 = 1 + D2 + D3 - D6 and
    // 2 q1.q3 = -D1 + D3 + D5 - D6 in the stable parent slot basis.
    let q1_q2 = &chart.routing.local_to_parent()[4];
    assert_eq!(q1_q2.constant(), &half);
    assert_eq!(
        q1_q2.denominator_coefficients(),
        [
            context.zero(),
            half.clone(),
            half.clone(),
            context.zero(),
            context.zero(),
            minus_half.clone(),
        ]
    );
    let q1_q3 = &chart.routing.local_to_parent()[5];
    assert_eq!(q1_q3.constant(), &context.zero());
    assert_eq!(
        q1_q3.denominator_coefficients(),
        [
            minus_half.clone(),
            context.zero(),
            half.clone(),
            context.zero(),
            half.clone(),
            minus_half.clone(),
        ]
    );

    let mut local_powers = vec![0_i64; ARITY];
    local_powers[4] = -33;
    local_powers[5] = -33;
    assert_eq!(
        chart.routing.try_admit_numerator_only_transport(
            &IntegralKey::try_new(local_powers).unwrap(),
            MatcherChartTransportLimits::new(64),
        ),
        Err(MatcherChartTransportError::AuxiliaryDegreeLimit {
            requested: 66,
            limit: 64,
        })
    );
}

#[test]
fn exact_matcher_routes_replay_stable_parent_slots_and_refuse_auxiliary_poles() {
    let first = MatcherSeedPortfolio::try_compile().unwrap();
    let second = MatcherSeedPortfolio::try_compile().unwrap();
    let parent = canonical_family().unwrap();
    let context = parent.coefficient_context();
    let transport_limits = MatcherChartTransportLimits::new(64);

    assert_eq!(first.charts.len(), second.charts.len());
    for (chart, repeated) in first.charts.iter().zip(second.charts.iter()) {
        assert_eq!(chart.routing, repeated.routing);
        assert_eq!(
            chart.completion.family().fingerprint(),
            repeated.completion.family().fingerprint()
        );
        assert_ne!(
            chart.completion.family().fingerprint(),
            parent.fingerprint()
        );

        let physical_count = chart.routing.physical_parent_slots().len();
        assert_eq!(physical_count, chart.completion.input_denominator_count());
        for (local_slot, &parent_slot) in chart.routing.physical_parent_slots().iter().enumerate() {
            let relation = &chart.routing.local_to_parent()[local_slot];
            assert_eq!(relation.constant(), &context.zero());
            assert_eq!(
                relation.denominator_coefficients(),
                &(0..ARITY)
                    .map(|candidate| {
                        if candidate == parent_slot {
                            context.one()
                        } else {
                            context.zero()
                        }
                    })
                    .collect::<Vec<_>>()
            );
        }

        // Distinct physical powers prove that transport preserves stable
        // parent slots rather than dense positions in the contracted chart.
        let mut local_powers = vec![0_i64; ARITY];
        let mut expected_parent = vec![0_i64; ARITY];
        for (local_slot, &parent_slot) in chart.routing.physical_parent_slots().iter().enumerate() {
            let power = 11 + i64::try_from(parent_slot).unwrap();
            local_powers[local_slot] = power;
            expected_parent[parent_slot] = power;
        }
        let admitted = chart
            .routing
            .try_admit_numerator_only_transport(
                &IntegralKey::try_new(local_powers.clone()).unwrap(),
                transport_limits,
            )
            .unwrap();
        assert_eq!(admitted.parent_physical_key().powers(), expected_parent);
        assert_eq!(admitted.total_auxiliary_numerator_degree(), 0);

        if physical_count < ARITY {
            local_powers[physical_count] = -1;
            let numerator = chart
                .routing
                .try_admit_numerator_only_transport(
                    &IntegralKey::try_new(local_powers.clone()).unwrap(),
                    transport_limits,
                )
                .unwrap();
            assert_eq!(numerator.total_auxiliary_numerator_degree(), 1);
            assert_eq!(
                chart.routing.try_admit_numerator_only_transport(
                    &IntegralKey::try_new(local_powers.clone()).unwrap(),
                    MatcherChartTransportLimits::new(0),
                ),
                Err(MatcherChartTransportError::AuxiliaryDegreeLimit {
                    requested: 1,
                    limit: 0,
                })
            );

            local_powers[physical_count] = 1;
            assert_eq!(
                chart.routing.try_admit_numerator_only_transport(
                    &IntegralKey::try_new(local_powers.clone()).unwrap(),
                    transport_limits,
                ),
                Err(MatcherChartTransportError::PositiveAuxiliaryPole {
                    local_slot: physical_count,
                    power: 1,
                })
            );

            local_powers[physical_count] = -65;
            assert_eq!(
                chart.routing.try_admit_numerator_only_transport(
                    &IntegralKey::try_new(local_powers.clone()).unwrap(),
                    transport_limits,
                ),
                Err(MatcherChartTransportError::AuxiliaryDegreeLimit {
                    requested: 65,
                    limit: 64,
                })
            );
        }
    }

    assert_eq!(
        first.charts[0].routing.try_admit_numerator_only_transport(
            &IntegralKey::try_new([0; 5]).unwrap(),
            transport_limits,
        ),
        Err(MatcherChartTransportError::WrongArity {
            expected: ARITY,
            actual: 5,
        })
    );
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
        assert_eq!(chart.ordinary.source_row_count(), 9);
        let local_sources = local_generator
            .translate_completed_source_rows(&chart.ordinary, [zero.clone()], limits.translation)
            .unwrap();
        assert_eq!(local_sources.family_fingerprint(), local.fingerprint());
        assert_ne!(local_sources.family_fingerprint(), parent.fingerprint());
    }
}

#[test]
fn matcher_roots_plus_structural_terminals_cover_the_complete_k6_sector_downset() {
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let authority = derive_k6_terminal_authority().unwrap();
    let seeded = portfolio
        .charts
        .iter()
        .map(|chart| chart.canonical_corner.clone())
        .collect::<BTreeSet<_>>();

    let mut terminal_owned_raw_sectors = 0usize;
    for (corner, raw_orbit_size) in &*portfolio.unseeded_required_orbits {
        assert!(!seeded.contains(corner));
        let zero_owned = authority.is_zero_terminal(corner);
        let factorization_owned = authority
            .factorization_rules()
            .iter()
            .any(|rule| rule.application_domain().contains(corner.powers()).unwrap());
        assert!(
            zero_owned || factorization_owned,
            "unseeded K6 orbit {corner:?} has neither a matcher chart nor structural terminal authority"
        );
        terminal_owned_raw_sectors += *raw_orbit_size;
    }

    // The apparent 30-sector gap is exactly 26 scaleless masks plus the
    // four-member K1^3 star orbit. It is not a sixth nonfactorized topology
    // that needs another ordinary-IBP chart.
    assert_eq!(terminal_owned_raw_sectors, 30);
    assert_eq!(
        portfolio.seeded_raw_sector_count + terminal_owned_raw_sectors,
        portfolio.complete_raw_sector_count
    );
}

#[test]
fn matcher_chart_degree_two_source_shapes_are_deterministic() {
    fn census(
        family: &IntegralFamily,
        sector: Mask,
    ) -> Result<(usize, usize, usize), Box<dyn std::error::Error>> {
        let generator =
            ParametricIbpGenerator::try_new_with_config(family, ParametricIbpConfig::default())?;
        let completed = complete_ordinary(&generator)?;
        let frame = OneSidedChartFrame::try_new(
            &generator,
            &completed,
            sector,
            2,
            PhysicalFrameLimits::default(),
        )?;
        Ok((
            frame.plan().row_count(),
            frame.plan().columns().len(),
            frame.plan().entry_count(),
        ))
    }

    let first = MatcherSeedPortfolio::try_compile().unwrap();
    let second = MatcherSeedPortfolio::try_compile().unwrap();
    let chart_census = |portfolio: &MatcherSeedPortfolio| {
        portfolio
            .charts
            .iter()
            .map(|chart| {
                let active = chart.completion.input_denominator_count();
                let sector = Mask::try_new((0..ARITY).map(|slot| slot < active)).unwrap();
                (
                    chart.diagnostic_label,
                    census(chart.completion.family(), sector).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    };
    let first_census = chart_census(&first);
    assert_eq!(first_census, chart_census(&second));

    // This is a measured scheduling fingerprint, not closure evidence. Every
    // chart still owns the same nine complete ordinary sources; only their
    // sparse coordinate representation and one-sided source neighbourhood
    // change.
    assert!(
        first_census
            .iter()
            .all(|(_, (rows, columns, entries))| *rows == 252 && *columns > 0 && *entries > 0)
    );
}
