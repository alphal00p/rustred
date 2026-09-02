//! Exact fixed-sample chart-transport regressions.

use crate::algebra::Coefficient;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::{
    MultiAffineNumeratorEndpoint, MultiAffineNumeratorExpansionLimits,
    try_expand_multi_affine_numerator,
};
use crate::identity::{
    IntegralShift, ParametricIbpConfig, ParametricIbpGenerator, TranslatedSourceLimits,
    TranslatedSourceRequest,
};
use crate::sector::OrderingPolicy;
use crate::sector::symmetry::permutation::compile;
use crate::sector::symmetry::{
    CanonicalizationLimits, Canonicalizer, CoefficientMatrix, Limits as SymmetryLimits,
    MomentumMap, verify,
};

use super::super::super::{canonical_family, canonical_s4};
use super::super::routing::{MatcherChartTransportError, MatcherChartTransportLimits};
use super::super::{MatcherSeedChart, MatcherSeedPortfolio};
use super::{
    FixedMatcherChartRowTransportError, FixedMatcherChartRowTransportLimits, numerator_factors,
    try_transport_fixed_matcher_chart_row,
};

const S5_SAMPLE_POWERS: [[i64; 6]; 4] = [
    [1, 1, 1, 1, 1, 0],
    [1, 1, 1, 1, 1, -1],
    [1, 1, 1, 1, 1, -2],
    [1, 1, 1, 1, 1, -4],
];

fn s5_chart(portfolio: &MatcherSeedPortfolio) -> &MatcherSeedChart {
    portfolio
        .charts
        .iter()
        .find(|chart| chart.diagnostic_label == "I3L_pinch_6")
        .expect("the frozen matcher portfolio contains the S5 chart")
}

fn s4_two_auxiliary_chart(portfolio: &MatcherSeedPortfolio) -> &MatcherSeedChart {
    portfolio
        .charts
        .iter()
        .find(|chart| chart.diagnostic_label == "I3L_pinch_1_6")
        .expect("the frozen matcher portfolio contains the two-auxiliary S4 chart")
}

fn expand_s5_auxiliary_power(
    parent: &IntegralFamily,
    chart: &MatcherSeedChart,
    degree: u64,
) -> Box<[MultiAffineNumeratorEndpoint]> {
    let degree = i64::try_from(degree).unwrap();
    let local = IntegralKey::try_new([1, 1, 1, 1, 1, -degree]).unwrap();
    let admission = chart
        .routing
        .try_admit_numerator_only_transport(
            &local,
            MatcherChartTransportLimits::new(degree.unsigned_abs()),
        )
        .unwrap();
    let factors = numerator_factors(chart, &local).unwrap();
    try_expand_multi_affine_numerator(
        parent,
        admission.parent_physical_key(),
        &factors,
        MultiAffineNumeratorExpansionLimits::default(),
    )
    .unwrap()
}

fn coefficient_at(endpoints: &[MultiAffineNumeratorEndpoint], powers: [i64; 6]) -> &Coefficient {
    let key = IntegralKey::try_new(powers).unwrap();
    endpoints
        .iter()
        .find(|endpoint| endpoint.key() == &key)
        .unwrap_or_else(|| panic!("missing exact S5 expansion endpoint {key:?}"))
        .coefficient()
}

#[test]
fn s5_auxiliary_samples_replay_two_s23_as_exact_symbolica_expansions() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let context = parent.coefficient_context();
    let algebra = parent.construction_limits().exact_algebra;

    // s23^r = (1 + D2 + D3 - D6)^r / 2^r. The native sparse
    // supports are C(r+3,3) for r = 0,1,2,4.
    for (degree, expected_support) in [(0_u64, 1_usize), (1, 4), (2, 10), (4, 35)] {
        let endpoints = expand_s5_auxiliary_power(&parent, chart, degree);
        assert_eq!(endpoints.len(), expected_support);
        assert!(
            endpoints
                .windows(2)
                .all(|pair| pair[0].key() < pair[1].key())
        );

        let denominator = context.integer(1_i64 << degree);
        let positive = context
            .try_div(&context.one(), &denominator, algebra)
            .unwrap();
        let signed_d6 = if degree % 2 == 0 {
            positive.clone()
        } else {
            context.try_neg(&positive, algebra).unwrap()
        };
        let degree_i64 = i64::try_from(degree).unwrap();
        assert_eq!(coefficient_at(&endpoints, [1, 1, 1, 1, 1, 0]), &positive);
        if degree != 0 {
            assert_eq!(
                coefficient_at(&endpoints, [1, 1 - degree_i64, 1, 1, 1, 0]),
                &positive
            );
            assert_eq!(
                coefficient_at(&endpoints, [1, 1, 1 - degree_i64, 1, 1, 0]),
                &positive
            );
            assert_eq!(
                coefficient_at(&endpoints, [1, 1, 1, 1, 1, -degree_i64]),
                &signed_d6
            );
        }

        // Evaluating every parent denominator symbol at one gives
        // ((1+1+1-1)/2)^r = 1, an independent exact coefficient checksum.
        let coefficient_sum = endpoints
            .iter()
            .try_fold(context.zero(), |sum, endpoint| {
                context.try_add(&sum, endpoint.coefficient(), algebra)
            })
            .unwrap();
        assert_eq!(coefficient_sum, context.one());
    }
}

#[test]
fn one_selected_s5_row_transports_the_zero_one_two_and_four_degree_samples() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();

    for sample in S5_SAMPLE_POWERS {
        let row = try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            IntegralShift::try_new(sample).unwrap(),
            FixedMatcherChartRowTransportLimits::default(),
        )
        .unwrap();
        assert_eq!(row.provenance().diagnostic_chart_label(), "I3L_pinch_6");
        assert_eq!(
            row.provenance().parent_family_fingerprint(),
            parent.fingerprint()
        );
        assert_eq!(
            row.provenance().local_family_fingerprint(),
            chart.completion.family().fingerprint()
        );
        assert_eq!(row.provenance().source_ordinal(), 0);
        assert_eq!(
            row.provenance().source_row(),
            chart.ordinary.source_row_id(0).unwrap()
        );
        assert_eq!(row.provenance().local_sample().values(), sample);
        assert_eq!(row.provenance().raw_target().powers(), [1, 1, 1, 1, 1, 0]);
        assert_eq!(
            row.provenance().canonical_target().powers(),
            [0, 1, 1, 1, 1, 1]
        );
        assert!(canonicalizer.authenticates_route(row.provenance().common_route()));
        assert!(row.provenance().common_route().verify(
            row.provenance().raw_target(),
            row.provenance().canonical_target()
        ));
        assert!(!row.terms().is_empty());
        assert!(row.telemetry().translated_terms() > 0);
        assert!(row.terms().windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            row.terms()
                .iter()
                .all(|(_, coefficient)| !coefficient.is_zero())
        );
        assert_eq!(row.support().len(), row.terms().len());
        assert_eq!(
            row.telemetry().canonical_parent_endpoints(),
            row.terms().len()
        );
        assert_eq!(
            row.telemetry().raw_parent_endpoints(),
            row.telemetry().canonical_parent_endpoints()
        );
        assert!(
            row.nonzero_conditions()
                .iter()
                .all(|guard| !guard.is_zero())
        );
    }
}

#[test]
fn one_s4_row_transports_two_live_auxiliary_numerators_together() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s4_two_auxiliary_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();
    let sample = IntegralShift::try_new([1, 1, 1, 1, -2, -2]).unwrap();
    let generator = ParametricIbpGenerator::try_new_with_config(
        chart.completion.family(),
        ParametricIbpConfig::default(),
    )
    .unwrap();
    let translated = generator
        .translate_selected_completed_source_rows(
            &chart.ordinary,
            [TranslatedSourceRequest::new(0, sample.clone())],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    assert!(translated.sources()[0].terms().keys().all(|key| {
        key.values()[4] < 0
            && key.values()[5] < 0
            && numerator_factors(
                chart,
                &IntegralKey::try_new(key.values().iter().copied()).unwrap(),
            )
            .unwrap()
            .len()
                == 2
    }));

    let row = try_transport_fixed_matcher_chart_row(
        &parent,
        chart,
        &canonicalizer,
        0,
        sample,
        FixedMatcherChartRowTransportLimits::default(),
    )
    .unwrap();
    assert_eq!(row.provenance().diagnostic_chart_label(), "I3L_pinch_1_6");
    assert_eq!(row.provenance().raw_target().powers(), [0, 1, 1, 1, 1, 0]);
    assert!(!row.terms().is_empty());
    assert!(row.telemetry().surviving_local_terms() > 0);
    assert!(
        row.telemetry().expanded_contributions() > row.telemetry().surviving_local_terms(),
        "each surviving local term has two nonconstant auxiliary factors"
    );
}

#[test]
fn translated_coefficients_are_specialized_at_zero_and_never_at_the_sample_twice() {
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let generator = ParametricIbpGenerator::try_new_with_config(
        chart.completion.family(),
        ParametricIbpConfig::default(),
    )
    .unwrap();
    let sample = IntegralShift::try_new([1, 1, 1, 1, 1, -2]).unwrap();
    let translated = generator
        .translate_selected_completed_source_rows(
            &chart.ordinary,
            [TranslatedSourceRequest::new(1, sample.clone())],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let original = chart.ordinary.source_relation(1).unwrap();
    let shifted = &translated.sources()[0];
    let zero = [0_i64; 6];
    let mut found_double_translation_witness = false;

    assert_eq!(original.terms().len(), shifted.terms().len());
    for ((original_shift, original_coefficient), (shifted_key, shifted_coefficient)) in
        original.terms().iter().zip(shifted.terms())
    {
        let expected_key = original_shift
            .values()
            .iter()
            .zip(sample.values())
            .map(|(&left, &right)| left.checked_add(right).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(shifted_key.values(), expected_key);

        let (direct_once, _) = generator
            .context()
            .specialize_sealed(original_coefficient, sample.values(), Default::default())
            .unwrap();
        let (translated_at_zero, _) = generator
            .context()
            .specialize_sealed(shifted_coefficient, &zero, Default::default())
            .unwrap();
        assert_eq!(translated_at_zero, direct_once);

        let (incorrectly_translated_twice, _) = generator
            .context()
            .specialize_sealed(shifted_coefficient, sample.values(), Default::default())
            .unwrap();
        found_double_translation_witness |= incorrectly_translated_twice != direct_once;
    }
    assert!(
        found_double_translation_witness,
        "the selected row must contain an index-dependent coefficient that detects n+s evaluated at s"
    );
}

#[test]
fn exact_zero_pruning_precedes_transactional_positive_auxiliary_pole_refusal() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();

    // Row one differentiates the local auxiliary denominator. At n6=0 its
    // would-be positive-pole terms carry an exact n6 coefficient and must be
    // removed before auxiliary admission.
    let boundary = try_transport_fixed_matcher_chart_row(
        &parent,
        chart,
        &canonicalizer,
        1,
        IntegralShift::try_new([1, 1, 1, 1, 1, 0]).unwrap(),
        FixedMatcherChartRowTransportLimits::default(),
    )
    .unwrap();
    assert!(boundary.telemetry().exact_zero_terms_pruned() > 0);
    assert!(boundary.telemetry().surviving_local_terms() > 0);

    // The same concrete row at a genuine positive auxiliary pole is refused
    // as one transaction; no partial cold row can be observed by the caller.
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            1,
            IntegralShift::try_new([1, 1, 1, 1, 1, 1]).unwrap(),
            FixedMatcherChartRowTransportLimits::default(),
        ),
        Err(FixedMatcherChartRowTransportError::ChartTransport(
            MatcherChartTransportError::PositiveAuxiliaryPole {
                local_slot: 5,
                power,
            }
        )) if power > 0
    ));
}

#[test]
fn complete_row_coalescing_removes_collisions_and_exact_cancellations() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();

    // Search a fixed, finite and deterministic census rather than asserting a
    // private source ordinal whose ordering is already pinned independently.
    let witness = [-1_i64, -2, -4].into_iter().find_map(|auxiliary| {
        (0..9).find_map(|source_ordinal| {
            let row = try_transport_fixed_matcher_chart_row(
                &parent,
                chart,
                &canonicalizer,
                source_ordinal,
                IntegralShift::try_new([1, 1, 1, 1, 1, auxiliary]).unwrap(),
                FixedMatcherChartRowTransportLimits::default(),
            )
            .ok()?;
            (row.telemetry().exact_parent_endpoint_cancellations() > 0).then_some(row)
        })
    });
    let row = witness.expect("the S5 fixed census contains a full-row coalescing witness");
    assert!(row.telemetry().exact_parent_endpoint_cancellations() > 0);
    assert!(row.telemetry().expanded_contributions() > row.telemetry().raw_parent_endpoints());
    assert_eq!(
        row.telemetry().raw_parent_endpoints(),
        row.telemetry().canonical_parent_endpoints()
    );
    assert!(row.terms().windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert!(
        row.terms()
            .iter()
            .all(|(_, coefficient)| !coefficient.is_zero())
    );
}

#[test]
fn one_authenticated_target_route_is_applied_to_the_correlated_row() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();

    let row = (0..9)
        .filter_map(|source_ordinal| {
            try_transport_fixed_matcher_chart_row(
                &parent,
                chart,
                &canonicalizer,
                source_ordinal,
                IntegralShift::try_new([1, 1, 1, 1, 1, -2]).unwrap(),
                FixedMatcherChartRowTransportLimits::default(),
            )
            .ok()
        })
        .find(|row| {
            row.support().any(|key| {
                canonicalizer
                    .canonicalize(key)
                    .is_ok_and(|image| image.canonical() != key)
            })
        })
        .expect("a correlated S5 row must expose why endpoint-wise canonicalization is invalid");
    let route = row.provenance().common_route();
    assert!(canonicalizer.authenticates_route(route));
    assert!(route.verify(
        row.provenance().raw_target(),
        row.provenance().canonical_target()
    ));
    assert!(
        row.support()
            .any(|key| { canonicalizer.canonicalize(key).unwrap().canonical() != key }),
        "uniform routing must retain correlated endpoints that an invalid per-endpoint minimum would move again"
    );

    // Invert the retained permutation value-wise and replay it forward for
    // every endpoint. One and the same authenticated mapping must work for
    // the complete row.
    for canonical_key in row.support() {
        let mut raw = vec![0_i64; canonical_key.powers().len()];
        for (target_slot, &source_slot) in route.source_for_target().iter().enumerate() {
            raw[source_slot] = canonical_key.powers()[target_slot];
        }
        let raw = IntegralKey::try_new(raw).unwrap();
        assert!(route.verify(&raw, canonical_key));
    }
}

fn identity_canonicalizer(family: &IntegralFamily) -> Canonicalizer {
    let loops = family.loop_count();
    let context = family.coefficient_context();
    let loop_entries = (0..loops)
        .flat_map(|row| {
            (0..loops).map(move |column| {
                if row == column {
                    context.one()
                } else {
                    context.zero()
                }
            })
        })
        .collect::<Vec<_>>();
    let map = MomentumMap::new(
        CoefficientMatrix::try_new(loops, loops, loop_entries).unwrap(),
        CoefficientMatrix::try_new(loops, 0, []).unwrap(),
        CoefficientMatrix::try_new(0, 0, []).unwrap(),
    );
    let verified = verify(family, family, map, SymmetryLimits::default()).unwrap();
    let permutation = compile(family, verified).unwrap();
    Canonicalizer::try_new(
        OrderingPolicy::default(),
        [permutation],
        CanonicalizationLimits::default(),
    )
    .unwrap()
}

#[test]
fn parent_canonicalizer_and_transport_resources_fail_with_typed_errors() {
    let parent = canonical_family().unwrap();
    let portfolio = MatcherSeedPortfolio::try_compile().unwrap();
    let chart = s5_chart(&portfolio);
    let canonicalizer = canonical_s4(&parent).unwrap();
    let sample = || IntegralShift::try_new([1, 1, 1, 1, 1, 0]).unwrap();

    assert_eq!(
        try_transport_fixed_matcher_chart_row(
            chart.completion.family(),
            chart,
            &canonicalizer,
            0,
            sample(),
            FixedMatcherChartRowTransportLimits::default(),
        ),
        Err(FixedMatcherChartRowTransportError::WrongParentFamily)
    );

    let foreign_canonicalizer = identity_canonicalizer(chart.completion.family());
    assert_eq!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &foreign_canonicalizer,
            0,
            sample(),
            FixedMatcherChartRowTransportLimits::default(),
        ),
        Err(FixedMatcherChartRowTransportError::WrongCanonicalizerFamily)
    );

    let mut limits = FixedMatcherChartRowTransportLimits::default();
    limits.max_surviving_local_terms = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart translated source terms",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut contribution_limits = FixedMatcherChartRowTransportLimits::default();
    contribution_limits.max_expanded_contributions = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            contribution_limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart expanded contributions",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut coalesced_limits = FixedMatcherChartRowTransportLimits::default();
    coalesced_limits.max_coalesced_parent_endpoints = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            coalesced_limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart coalesced parent endpoints",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut coefficient_term_limits = FixedMatcherChartRowTransportLimits::default();
    coefficient_term_limits.max_retained_parent_coefficient_terms = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            coefficient_term_limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart retained coefficient terms",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut coefficient_byte_limits = FixedMatcherChartRowTransportLimits::default();
    coefficient_byte_limits.max_retained_parent_coefficient_clone_owned_bytes = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            coefficient_byte_limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart retained coefficient clone-owned bytes",
            requested,
            limit: 0,
        }) if requested > 0
    ));

    let mut route_limits = FixedMatcherChartRowTransportLimits::default();
    route_limits.max_common_route_coordinate_cells = 0;
    assert!(matches!(
        try_transport_fixed_matcher_chart_row(
            &parent,
            chart,
            &canonicalizer,
            0,
            sample(),
            route_limits,
        ),
        Err(FixedMatcherChartRowTransportError::ResourceLimit {
            resource: "matcher-chart common-route coordinate cells",
            requested,
            limit: 0,
        }) if requested > 0
    ));
}
