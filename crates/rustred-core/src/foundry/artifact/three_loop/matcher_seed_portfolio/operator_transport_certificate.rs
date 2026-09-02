//! Exact operator-level certificate for compact matcher-chart parent seeds.
//!
//! If `q = T k`, then the ordinary IBP operator transforms as
//!
//! `q_a . d/dq_b = sum_{i,j} T[a,i] (T^-1)[j,b] k_i . d/dk_j`.
//!
//! For a local sample with zero auxiliary powers this expresses every chart
//! row through at most `L^2` parent ordinary rows at one and the same offset.
//! The test below replays both sides independently through the sealed source
//! chronologies and compares their exact Symbolica coefficients after the
//! chart's denominator transport.  It is deliberately stronger than a
//! support comparison and deliberately much smaller than inverse incidence
//! over every output endpoint.

use std::collections::BTreeMap;

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::IntegralKey;
use crate::foundry::completion::source_discovery::test_fixtures::OracleDisabledK6Fixture;
use crate::foundry::completion::source_discovery::{
    OrdinarySourceIncidenceIndex, SourceDiscoveryLimits,
};
use crate::identity::{IntegralShift, TranslatedSourceRequest};

use super::super::{canonical_family, canonical_s4};
use super::transport::{
    ColdFixedMatcherChartParentRow, FixedMatcherChartRowTransportLimits,
    try_transport_fixed_matcher_chart_row,
};
use super::{MatcherSeedChart, MatcherSeedPortfolio};

const LOOP_COUNT: usize = 3;
const CHART_LABEL: &str = "I3L_pinch_1_6";
const S4A_SAMPLES: [[i64; 6]; 2] = [[1, 1, 2, 4, 0, 0], [1, 1, 2, 5, 0, 0]];

fn s4a_chart(portfolio: &MatcherSeedPortfolio) -> &MatcherSeedChart {
    portfolio
        .charts
        .iter()
        .find(|chart| chart.diagnostic_label == CHART_LABEL)
        .expect("the frozen matcher portfolio contains the natural S4a chart")
}

fn operator_weights(
    context: &CoefficientContext,
    chart: &MatcherSeedChart,
    local_source_ordinal: usize,
) -> BTreeMap<usize, Coefficient> {
    let contraction = local_source_ordinal / LOOP_COUNT;
    let differentiated = local_source_ordinal % LOOP_COUNT;
    let limits = chart
        .completion
        .family()
        .construction_limits()
        .exact_algebra;
    let mut weights = BTreeMap::new();
    for parent_contraction in 0..LOOP_COUNT {
        let left = context
            .integer(chart.routing.loop_basis()[contraction * LOOP_COUNT + parent_contraction]);
        if left.is_zero() {
            continue;
        }
        for parent_differentiated in 0..LOOP_COUNT {
            let right = &chart.routing.inverse_loop_basis()
                [parent_differentiated * LOOP_COUNT + differentiated];
            if right.is_zero() {
                continue;
            }
            let weight = context.try_mul(&left, right, limits).unwrap();
            if !weight.is_zero() {
                let parent_source_ordinal = parent_contraction * LOOP_COUNT + parent_differentiated;
                assert!(weights.insert(parent_source_ordinal, weight).is_none());
            }
        }
    }
    assert!(!weights.is_empty());
    assert!(weights.len() <= LOOP_COUNT * LOOP_COUNT);
    weights
}

fn accumulate(
    context: &CoefficientContext,
    terms: &mut BTreeMap<IntegralKey, Coefficient>,
    key: IntegralKey,
    contribution: Coefficient,
) {
    if contribution.is_zero() {
        return;
    }
    let limits = Default::default();
    match terms.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(contribution);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let sum = context.try_add(entry.get(), &contribution, limits).unwrap();
            if sum.is_zero() {
                entry.remove();
            } else {
                *entry.get_mut() = sum;
            }
        }
    }
}

fn replay_parent_operator_combination(
    fixture: &OracleDisabledK6Fixture,
    parent_context: &CoefficientContext,
    chart: &MatcherSeedChart,
    local_source_ordinal: usize,
    parent_target: &IntegralShift,
) -> (
    Vec<TranslatedSourceRequest>,
    Vec<(IntegralKey, Coefficient)>,
) {
    assert!(parent_context.has_same_variable_map(fixture.generator().context().base()));
    let weights = operator_weights(parent_context, chart, local_source_ordinal);
    let requests = weights
        .keys()
        .map(|&ordinal| TranslatedSourceRequest::new(ordinal, parent_target.clone()))
        .collect::<Vec<_>>();
    let selected = fixture
        .generator()
        .translate_selected_completed_source_rows(
            fixture.completed(),
            requests.iter().cloned(),
            Default::default(),
        )
        .unwrap();
    assert_eq!(selected.requests(), requests);

    let zero = [0_i64; 6];
    let mut exact = BTreeMap::new();
    for source in selected.sources() {
        assert!(source.nonzero_conditions().is_empty());
        let source_ordinal = source.provenance().source_ordinal();
        let weight = weights
            .get(&source_ordinal)
            .expect("every regenerated parent row has an operator weight");
        for (shift, indexed_coefficient) in source.terms() {
            let (coefficient, denominator_condition) = fixture
                .generator()
                .context()
                .specialize_sealed(indexed_coefficient, &zero, Default::default())
                .unwrap();
            assert!(denominator_condition.is_none());
            let contribution = parent_context
                .try_mul(weight, &coefficient, Default::default())
                .unwrap();
            accumulate(
                parent_context,
                &mut exact,
                IntegralKey::try_new(shift.values().iter().copied()).unwrap(),
                contribution,
            );
        }
    }
    (requests, exact.into_iter().collect())
}

fn assert_identity_route(row: &ColdFixedMatcherChartParentRow) {
    assert_eq!(
        row.provenance().raw_target(),
        row.provenance().canonical_target()
    );
    assert_eq!(
        row.provenance().common_route().source_for_target(),
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn every_natural_s4a_row_is_exactly_certified_by_at_most_nine_parent_rows() {
    let parent = canonical_family().unwrap();
    let canonicalizer = canonical_s4(&parent).unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s4a_chart(&portfolio);
    let fixture = OracleDisabledK6Fixture::shared();
    let mut union = BTreeMap::<TranslatedSourceRequest, ()>::new();
    let incidence = OrdinarySourceIncidenceIndex::try_new(
        fixture.zero_sources(),
        SourceDiscoveryLimits::default(),
    )
    .unwrap();

    for sample in S4A_SAMPLES {
        let local_sample = IntegralShift::try_new(sample).unwrap();
        for local_source_ordinal in 0..LOOP_COUNT * LOOP_COUNT {
            let transported = try_transport_fixed_matcher_chart_row(
                &parent,
                chart,
                &canonicalizer,
                local_source_ordinal,
                local_sample.clone(),
                FixedMatcherChartRowTransportLimits::default(),
            )
            .unwrap();
            assert_identity_route(&transported);
            let parent_target = IntegralShift::try_new(
                transported
                    .provenance()
                    .raw_target()
                    .powers()
                    .iter()
                    .copied(),
            )
            .unwrap();
            let (requests, replayed) = replay_parent_operator_combination(
                fixture,
                parent.coefficient_context(),
                chart,
                local_source_ordinal,
                &parent_target,
            );
            assert!(requests.len() <= LOOP_COUNT * LOOP_COUNT);
            for request in requests {
                union.insert(request, ());
            }
            assert_eq!(replayed.as_slice(), transported.terms());
        }

        let parent_target =
            IntegralShift::try_new([0, sample[0], sample[1], sample[2], sample[3], 0]).unwrap();
        let baseline = incidence
            .try_nominate_target_unit(&parent_target, SourceDiscoveryLimits::default())
            .unwrap();
        let direct = (0..LOOP_COUNT * LOOP_COUNT)
            .map(|ordinal| TranslatedSourceRequest::new(ordinal, parent_target.clone()))
            .collect::<Vec<_>>();
        let overlap = direct
            .iter()
            .filter(|request| baseline.requests().binary_search(request).is_ok())
            .count();
        eprintln!(
            "S4a compact operator certificate target={:?}: direct={}, target-unit overlap={overlap}",
            parent_target.values(),
            direct.len(),
        );
    }

    // All nine local operators together span the same nine parent operators
    // at each one of the two concrete offsets: no endpoint inverse-incidence
    // fan-out is needed.
    assert_eq!(union.len(), 2 * LOOP_COUNT * LOOP_COUNT);
}
