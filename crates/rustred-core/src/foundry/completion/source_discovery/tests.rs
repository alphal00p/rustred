use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::foundry::completion::frame::modular::{ModularKernelLimits, ModularTargetQuery};
use crate::foundry::completion::frame::{OneSidedChartFrame, PhysicalFrameLimits};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, TranslatedSourceBatch,
    TranslatedSourceLimits, TranslatedSourceRequest,
};
use crate::sector::Mask;

use super::nominate::nominate_support_for_test;
use super::{OrdinarySourceIncidenceIndex, SourceDiscoveryError, SourceDiscoveryLimits};

const PRIME: u64 = 1_000_000_007;

fn complete_ordinary(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_ordinary_ibp().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn complete_external(generator: &ParametricIbpGenerator<'_>) -> CompletedIbpSourceRows {
    let prepared = generator.prepare_external_ibp_sources().unwrap();
    let rows = (0..prepared.len())
        .map(|ordinal| prepared.generate(ordinal))
        .collect();
    prepared.complete(rows).unwrap()
}

fn one_loop_one_external(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        vec!["p".to_owned()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            AffineDenominator::new(context.integer(-1), vec![context.one(), context.zero()]),
            AffineDenominator::new(context.zero(), vec![context.zero(), context.one()]),
        ],
        vec![vec![context.parameter("s").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

fn zero_offset_sources(
    generator: &ParametricIbpGenerator<'_>,
    completed: &CompletedIbpSourceRows,
) -> TranslatedSourceBatch {
    generator
        .translate_completed_source_rows(
            completed,
            [IntegralShift::try_new([0; 6]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap()
}

#[test]
fn incidence_accepts_complete_ordinary_and_rejects_external_only_zero_offset_sources() {
    let family = one_loop_one_external("source-discovery-layout-seal");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let translate = |completed: &CompletedIbpSourceRows| {
        generator
            .translate_completed_source_rows(
                completed,
                [IntegralShift::try_new([0, 0]).unwrap()],
                TranslatedSourceLimits::default(),
            )
            .unwrap()
    };

    let ordinary = complete_ordinary(&generator);
    let ordinary = translate(&ordinary);
    assert!(ordinary.is_complete_ordinary());
    OrdinarySourceIncidenceIndex::try_new(&ordinary, SourceDiscoveryLimits::default()).unwrap();

    let external = complete_external(&generator);
    let external = translate(&external);
    assert!(!external.is_complete_ordinary());
    assert!(matches!(
        OrdinarySourceIncidenceIndex::try_new(&external, SourceDiscoveryLimits::default()),
        Err(SourceDiscoveryError::WrongSourceLayout {
            actual: "external-contraction IBP source",
        })
    ));
}

#[test]
fn canonical_k6_index_and_target_unit_bootstrap_have_frozen_census() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = zero_offset_sources(&generator, &completed);
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();

    assert_eq!(incidence.arity(), 6);
    assert_eq!(incidence.source_count(), 9);
    assert_eq!(incidence.term_occurrences(), 90);
    assert_eq!(incidence.distinct_shift_count(), 31);

    let target = IntegralShift::try_new([0; 6]).unwrap();
    let first = incidence.try_nominate_target_unit(&target, limits).unwrap();
    let repeated = incidence.try_nominate_target_unit(&target, limits).unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.raw_incidence_visits(), 90);
    assert_eq!(first.unique_before_existing_exclusion(), 90);
    assert_eq!(first.excluded_existing_requests(), 0);
    assert_eq!(first.requests().len(), 90);
    assert!(first.requests().windows(2).all(|pair| pair[0] < pair[1]));
}

#[test]
fn inverse_incidence_uses_exact_alpha_and_deduplicates_multiple_witnesses() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = zero_offset_sources(&generator, &completed);
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let source = &sources.sources()[0];
    let mut shifts = source.terms().keys();
    let first_shift = shifts.next().unwrap();
    let second_shift = shifts.next().unwrap();
    let alpha = [2, -1, 0, 1, -2, 3];
    let first_support = shifted_support(first_shift.values(), &alpha);
    let second_support = shifted_support(second_shift.values(), &alpha);

    let forward =
        nominate_support_for_test(&incidence, &[&first_support, &second_support], &[], limits)
            .unwrap();
    let reversed =
        nominate_support_for_test(&incidence, &[&second_support, &first_support], &[], limits)
            .unwrap();
    assert_eq!(forward, reversed);
    assert_eq!(forward.raw_incidence_visits(), 180);
    assert!(forward.unique_before_existing_exclusion() < 180);

    let expected = TranslatedSourceRequest::new(0, IntegralShift::try_new(alpha).unwrap());
    assert_eq!(
        forward
            .requests()
            .iter()
            .filter(|request| *request == &expected)
            .count(),
        1
    );
}

#[test]
fn checked_subtraction_rejects_unrepresentable_incidence_offsets() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = zero_offset_sources(&generator, &completed);
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let (position, sign) = sources
        .sources()
        .iter()
        .flat_map(|source| source.terms().keys())
        .find_map(|shift| {
            shift
                .values()
                .iter()
                .enumerate()
                .find(|(_, value)| **value != 0)
                .map(|(position, &value)| (position, value.signum()))
        })
        .unwrap();
    let mut support = [0; 6];
    support[position] = if sign > 0 { i64::MIN } else { i64::MAX };
    let support = IntegralShift::try_new(support).unwrap();

    assert!(matches!(
        nominate_support_for_test(&incidence, &[&support], &[], limits),
        Err(SourceDiscoveryError::ShiftOverflow { .. })
    ));
}

#[test]
fn index_and_nomination_limits_admit_boundaries_and_reject_one_below() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = zero_offset_sources(&generator, &completed);
    let exact = SourceDiscoveryLimits::default();

    for (limits, resource, requested, limit) in [
        (
            {
                let mut value = exact;
                value.max_arity = 5;
                value
            },
            "source-discovery arity",
            6,
            5,
        ),
        (
            {
                let mut value = exact;
                value.max_source_rows = 8;
                value
            },
            "source-discovery ordinary source rows",
            9,
            8,
        ),
        (
            {
                let mut value = exact;
                value.max_source_term_occurrences = 89;
                value
            },
            "source-discovery ordinary term occurrences",
            90,
            89,
        ),
        (
            {
                let mut value = exact;
                value.max_distinct_source_shifts = 30;
                value
            },
            "source-discovery distinct ordinary shifts",
            31,
            30,
        ),
    ] {
        assert_eq!(
            OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap_err(),
            SourceDiscoveryError::ResourceLimit {
                resource,
                requested,
                limit,
            }
        );
    }

    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, exact).unwrap();
    let target = IntegralShift::try_new([0; 6]).unwrap();
    for (limits, resource, requested, limit) in [
        (
            {
                let mut value = exact;
                value.max_obstruction_support = 0;
                value
            },
            "source-discovery obstruction support entries",
            1,
            0,
        ),
        (
            {
                let mut value = exact;
                value.max_incidence_visits = 89;
                value
            },
            "source-discovery inverse-incidence visits",
            90,
            89,
        ),
        (
            {
                let mut value = exact;
                value.max_candidate_coordinate_cells = 539;
                value
            },
            "source-discovery candidate coordinate cells",
            540,
            539,
        ),
        (
            {
                let mut value = exact;
                value.max_raw_requests = 89;
                value
            },
            "source-discovery raw translated-source requests",
            90,
            89,
        ),
        (
            {
                let mut value = exact;
                value.max_unique_requests = 89;
                value
            },
            "source-discovery unique translated-source requests",
            90,
            89,
        ),
    ] {
        assert_eq!(
            incidence
                .try_nominate_target_unit(&target, limits)
                .unwrap_err(),
            SourceDiscoveryError::ResourceLimit {
                resource,
                requested,
                limit,
            }
        );
    }

    let mut boundary = exact;
    boundary.max_arity = 6;
    boundary.max_source_rows = 9;
    boundary.max_source_term_occurrences = 90;
    boundary.max_distinct_source_shifts = 31;
    boundary.max_obstruction_support = 1;
    boundary.max_incidence_visits = 90;
    boundary.max_candidate_coordinate_cells = 540;
    boundary.max_raw_requests = 90;
    boundary.max_unique_requests = 90;
    let bounded = OrdinarySourceIncidenceIndex::try_new(&sources, boundary).unwrap();
    assert_eq!(
        bounded
            .try_nominate_target_unit(&target, boundary)
            .unwrap()
            .requests()
            .len(),
        90
    );
}

#[test]
fn checked_obstruction_nomination_excludes_every_materialized_request() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let context = generator.context().clone();
    let completed = complete_ordinary(&generator);
    let sources = zero_offset_sources(&generator, &completed);
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();

    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([false, true, true, true, true, false]).unwrap(),
        1,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let plan = chart.plan();
    let sampled = plan
        .try_modular_sample(
            &context,
            PRIME,
            &[37],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let forbidden = (1..plan.columns().len()).collect::<Vec<_>>();
    let query = sampled
        .query_target(0, &forbidden, ModularKernelLimits::default())
        .unwrap();
    let ModularTargetQuery::NoHitWithObstruction(obstruction) = query else {
        panic!("canonical all-other-column K6 query must have a checked obstruction")
    };

    let mut below_support = limits;
    below_support.max_obstruction_support = obstruction.entries().len() - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction(&obstruction, below_support)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction support entries",
            requested: obstruction.entries().len(),
            limit: obstruction.entries().len() - 1,
        }
    );
    let mut below_existing = limits;
    below_existing.max_existing_requests = plan.source_instances().len() - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction(&obstruction, below_existing)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery existing translated-source requests",
            requested: plan.source_instances().len(),
            limit: plan.source_instances().len() - 1,
        }
    );

    let mut exact = limits;
    exact.max_obstruction_support = obstruction.entries().len();
    exact.max_existing_requests = plan.source_instances().len();
    let first = incidence
        .try_nominate_obstruction(&obstruction, exact)
        .unwrap();
    let repeated = incidence
        .try_nominate_obstruction(&obstruction, exact)
        .unwrap();
    assert_eq!(first, repeated);
    assert!(first.excluded_existing_requests() > 0);
    for request in first.requests() {
        assert!(!plan.source_instances().iter().any(|source| {
            source.provenance().source_ordinal() == request.source_ordinal()
                && source.provenance().offset() == request.offset()
        }));
    }
}

fn shifted_support(source: &[i64], alpha: &[i64; 6]) -> IntegralShift {
    IntegralShift::try_new(
        source
            .iter()
            .zip(alpha)
            .map(|(&source, &alpha)| source.checked_add(alpha).unwrap()),
    )
    .unwrap()
}
