use std::collections::BTreeMap;
use std::sync::Arc;

use symbolica::domains::finite_field::FiniteFieldCore;
use symbolica::domains::{Ring, RingOps};

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};
use crate::foundry::artifact::canonical_three_loop_family;
use crate::foundry::completion::frame::modular::{
    ModularKernelLimits, ModularPhysicalFrame, ModularRightObstruction,
    ModularSourceEvaluationError, ModularTargetQuery,
};
use crate::foundry::completion::frame::{OneSidedChartFrame, PhysicalFrameLimits};
use crate::identity::{
    CompletedIbpSourceRows, IntegralShift, ParametricIbpGenerator, RowId, TranslatedSourceBatch,
    TranslatedSourceError, TranslatedSourceLimits, TranslatedSourceRequest,
};
use crate::sector::Mask;

use super::nominate::{empty_obstruction_nominations_for_test, nominate_support_for_test};
use super::residual::pair_selected_sources_for_test;
use super::{
    OrdinarySourceIncidenceIndex, ProbeRowEvaluationCache, SourceDiscoveryError,
    SourceDiscoveryLimits,
};

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

fn one_loop_vacuum(name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        name,
        vec!["k".to_owned()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            context.integer(-1),
            vec![context.one()],
        )],
        Vec::new(),
        vec![context.zero()],
    )
    .unwrap()
}

fn all_other_no_hit<'frame>(
    sampled: &ModularPhysicalFrame<'frame>,
) -> ModularRightObstruction<'frame> {
    for target in 0..sampled.plan().columns().len() {
        let forbidden = (0..sampled.plan().columns().len())
            .filter(|&column| column != target)
            .collect::<Vec<_>>();
        if let ModularTargetQuery::NoHitWithObstruction(obstruction) = sampled
            .query_target(target, &forbidden, ModularKernelLimits::default())
            .unwrap()
        {
            return obstruction;
        }
    }
    panic!("fixture has no all-other-column modular no-hit")
}

fn different_target_all_other_no_hit<'frame>(
    sampled: &ModularPhysicalFrame<'frame>,
    excluded_target: usize,
) -> ModularRightObstruction<'frame> {
    for target in 0..sampled.plan().columns().len() {
        if target == excluded_target {
            continue;
        }
        let forbidden = (0..sampled.plan().columns().len())
            .filter(|&column| column != target)
            .collect::<Vec<_>>();
        if let ModularTargetQuery::NoHitWithObstruction(obstruction) = sampled
            .query_target(target, &forbidden, ModularKernelLimits::default())
            .unwrap()
        {
            return obstruction;
        }
    }
    panic!("fixture has no second all-other-column modular no-hit")
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

#[test]
fn obstruction_block_union_is_deterministic_and_retains_primary_as_exact_subset() {
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
        .query_target_with_obstruction_rotation(0, &forbidden, 2, ModularKernelLimits::default())
        .unwrap();
    let ModularTargetQuery::NoHitWithObstruction(obstruction) = query else {
        panic!("canonical all-other-column K6 query must have a checked obstruction")
    };

    let primary = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();
    let first = incidence
        .try_nominate_obstruction_block(&obstruction, &primary, limits)
        .unwrap();
    let repeated = incidence
        .try_nominate_obstruction_block(&obstruction, &primary, limits)
        .unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.primary().requests(), first.union().primary_requests());
    assert!(
        first.primary().requests().iter().all(|request| first
            .union()
            .requests()
            .binary_search(request)
            .is_ok())
    );
    assert_eq!(
        first.union().direction_count(),
        obstruction.proposal_block().directions().len()
    );
    assert!(first.union().direction_count() <= 4);

    // Reconstruct each member's exact raw support independently and compare
    // it to the corresponding dense coordinate of the canonical union.
    for (direction_ordinal, direction) in
        obstruction.proposal_block().directions().iter().enumerate()
    {
        let direct = direction
            .entries()
            .iter()
            .map(|entry| {
                let physical = obstruction.logical_physical_columns()[entry.logical_column()];
                (
                    plan.columns()[physical].clone(),
                    entry.coefficient().clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let from_union = first
            .union()
            .support()
            .iter()
            .filter_map(|entry| {
                let coefficient = &entry.coefficients()[direction_ordinal];
                (!sampled.field().is_zero(coefficient))
                    .then(|| (entry.shift().clone(), coefficient.clone()))
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(from_union, direct);
    }

    let raw_block_entries = obstruction
        .proposal_block()
        .directions()
        .iter()
        .map(|direction| direction.entries().len())
        .sum::<usize>();
    let upper = first.union().nomination_upper_bound();
    assert_eq!(upper.raw_block_entries(), raw_block_entries);
    let exact_union_visits = upper.raw_request_visits();
    let mut exact = limits;
    exact.max_union_block_entries = raw_block_entries;
    exact.max_union_support_entries = raw_block_entries;
    exact.max_union_support_coordinate_cells = raw_block_entries * incidence.arity();
    exact.max_union_support_coefficient_cells = raw_block_entries * first.union().direction_count();
    exact.max_union_incidence_visits = exact_union_visits;
    exact.max_union_raw_requests = exact_union_visits;
    exact.max_union_unique_requests = first.union().unique_before_existing_exclusion();
    exact.max_union_request_coordinate_cells = exact_union_visits * incidence.arity();
    exact.max_union_subset_comparisons = upper.subset_comparisons();
    exact.max_union_canonicalization_logical_work_reservation =
        upper.canonicalization_logical_work_reservation();
    assert_eq!(
        incidence
            .try_nominate_obstruction_block(&obstruction, &primary, exact)
            .unwrap(),
        first
    );

    let mut below_block = exact;
    below_block.max_union_block_entries = raw_block_entries - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction_block(&obstruction, &primary, below_block)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block raw entries",
            requested: raw_block_entries,
            limit: raw_block_entries - 1,
        }
    );
    let mut below_pairing = exact;
    below_pairing.max_union_incidence_visits = exact_union_visits - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction_block(&obstruction, &primary, below_pairing)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block union incidence visits",
            requested: exact_union_visits,
            limit: exact_union_visits - 1,
        }
    );
    let mut no_subset_work = exact;
    no_subset_work.max_union_subset_comparisons = upper.subset_comparisons() - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction_block(&obstruction, &primary, no_subset_work)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block primary-subset comparisons",
            requested: upper.subset_comparisons(),
            limit: upper.subset_comparisons() - 1,
        }
    );
    let mut below_sort_work = exact;
    below_sort_work.max_union_canonicalization_logical_work_reservation =
        upper.canonicalization_logical_work_reservation() - 1;
    assert_eq!(
        incidence
            .try_nominate_obstruction_block(&obstruction, &primary, below_sort_work)
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery obstruction-block canonicalization logical-work reservation",
            requested: upper.canonicalization_logical_work_reservation(),
            limit: upper.canonicalization_logical_work_reservation() - 1,
        }
    );
}

#[test]
fn probe_row_cache_retains_complete_zero_values_and_rejects_foreign_scope() {
    let family = canonical_three_loop_family().unwrap();
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
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
            generator.context(),
            PRIME,
            &[37],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();

    let mut direct = Vec::new();
    let (source, request, expected) = (0..plan.row_count())
        .find_map(|row| {
            let source = plan.source_for_row(row).unwrap();
            sampled
                .try_evaluate_translated_source(generator.context(), source, &mut direct)
                .unwrap();
            direct
                .iter()
                .any(|value| sampled.field().is_zero(value))
                .then(|| {
                    (
                        source,
                        TranslatedSourceRequest::new(
                            source.provenance().source_ordinal(),
                            source.provenance().offset().clone(),
                        ),
                        direct.clone(),
                    )
                })
        })
        .expect("the K6 sample must contain an explicit modular-zero row value");
    assert!(expected.iter().any(|value| sampled.field().is_zero(value)));

    let mut cache = ProbeRowEvaluationCache::try_new(&incidence, &completed, limits).unwrap();
    let cold = cache
        .try_evaluate(
            &incidence,
            generator.context(),
            &request,
            source,
            &sampled,
            0,
            limits,
        )
        .unwrap();
    assert_eq!(cold.as_ref(), expected.as_slice());
    assert_eq!(cache.telemetry().rows(), 1);
    assert_eq!(cache.telemetry().value_cells(), expected.len());
    assert_eq!(cache.telemetry().physical_evaluations(), 1);
    assert_eq!(cache.telemetry().cache_hits(), 0);

    // A fresh sampled owner for the same exact modulus/point is a valid hit;
    // pointer identity is deliberately not used as a computation-cache key.
    let repeated_sample = plan
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[1, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let warm = cache
        .try_evaluate(
            &incidence,
            generator.context(),
            &request,
            source,
            &repeated_sample,
            0,
            limits,
        )
        .unwrap();
    assert_eq!(warm, cold);
    assert_eq!(cache.telemetry().physical_evaluations(), 1);
    assert_eq!(cache.telemetry().cache_hits(), 1);
    assert!(cache.telemetry().lookup_comparisons() > 0);

    let foreign_incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    assert!(matches!(
        cache.try_evaluate(
            &foreign_incidence,
            generator.context(),
            &request,
            source,
            &sampled,
            0,
            limits,
        ),
        Err(SourceDiscoveryError::ScopeMismatch {
            detail: "row cache belongs to a different incidence owner"
        })
    ));
    let foreign_point = plan
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[2, 2, 3, 4, 5, 6],
            ModularKernelLimits::default(),
        )
        .unwrap();
    assert!(matches!(
        cache.try_evaluate(
            &incidence,
            generator.context(),
            &request,
            source,
            &foreign_point,
            0,
            limits,
        ),
        Err(SourceDiscoveryError::ScopeMismatch {
            detail: "row cache modulus or complete evaluation point changed within one probe"
        })
    ));
    assert_eq!(cache.telemetry().rows(), 1);
    assert_eq!(cache.telemetry().physical_evaluations(), 1);

    let mut no_rows = limits;
    no_rows.max_row_cache_rows = 0;
    let mut bounded = ProbeRowEvaluationCache::try_new(&incidence, &completed, no_rows).unwrap();
    assert_eq!(
        bounded
            .try_evaluate(
                &incidence,
                generator.context(),
                &request,
                source,
                &sampled,
                0,
                no_rows,
            )
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery probe row-cache rows",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(bounded.telemetry().rows(), 0);
    assert_eq!(bounded.telemetry().physical_evaluations(), 0);

    let mut ordered_rows = (0..plan.row_count())
        .map(|row| {
            let source = plan.source_for_row(row).unwrap();
            (
                TranslatedSourceRequest::new(
                    source.provenance().source_ordinal(),
                    source.provenance().offset().clone(),
                ),
                row,
            )
        })
        .collect::<Vec<_>>();
    ordered_rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    ordered_rows.dedup_by(|left, right| left.0 == right.0);
    let (low_request, low_row) = ordered_rows.first().cloned().unwrap();
    let (high_request, high_row) = ordered_rows.last().cloned().unwrap();
    assert!(low_request < high_request);
    let low_source = plan.source_for_row(low_row).unwrap();
    let high_source = plan.source_for_row(high_row).unwrap();

    let mut exact_moves = limits;
    exact_moves.max_row_cache_insertion_moves = 1;
    let mut descending =
        ProbeRowEvaluationCache::try_new(&incidence, &completed, exact_moves).unwrap();
    descending
        .try_evaluate(
            &incidence,
            generator.context(),
            &high_request,
            high_source,
            &sampled,
            0,
            exact_moves,
        )
        .unwrap();
    descending
        .try_evaluate(
            &incidence,
            generator.context(),
            &low_request,
            low_source,
            &sampled,
            1,
            exact_moves,
        )
        .unwrap();
    assert_eq!(descending.telemetry().insertion_moves(), 1);

    let mut below_moves = exact_moves;
    below_moves.max_row_cache_insertion_moves = 0;
    let mut bounded_moves =
        ProbeRowEvaluationCache::try_new(&incidence, &completed, below_moves).unwrap();
    bounded_moves
        .try_evaluate(
            &incidence,
            generator.context(),
            &high_request,
            high_source,
            &sampled,
            0,
            below_moves,
        )
        .unwrap();
    assert_eq!(
        bounded_moves
            .try_evaluate(
                &incidence,
                generator.context(),
                &low_request,
                low_source,
                &sampled,
                1,
                below_moves,
            )
            .unwrap_err(),
        SourceDiscoveryError::ResourceLimit {
            resource: "source-discovery probe row-cache insertion moves",
            requested: 1,
            limit: 0,
        }
    );
    assert_eq!(bounded_moves.telemetry().rows(), 1);
    assert_eq!(bounded_moves.telemetry().physical_evaluations(), 1);
    assert_eq!(bounded_moves.telemetry().insertion_moves(), 0);
}

#[test]
fn complete_row_residual_pairing_matches_an_independent_raw_shift_replay() {
    let family = one_loop_vacuum("source-discovery-residual-replay");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();
    let retained = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            limits,
        )
        .unwrap();
    let repeated = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            limits,
        )
        .unwrap();
    assert_eq!(retained, repeated);

    let selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.translation,
        )
        .unwrap();
    let mut raw_q = BTreeMap::new();
    for entry in obstruction.entries() {
        let physical = obstruction.logical_physical_columns()[entry.logical_column()];
        raw_q.insert(
            sampled.plan().columns()[physical].values().to_vec(),
            entry.coefficient().clone(),
        );
    }
    let mut evaluated = Vec::new();
    let mut expected = Vec::new();
    let mut paired = 0usize;
    for (request, source) in selected.requests().iter().zip(selected.sources()) {
        sampled
            .try_evaluate_translated_source(generator.context(), source, &mut evaluated)
            .unwrap();
        assert_eq!(evaluated.len(), source.terms().len());
        let mut residual = sampled.field().zero();
        for (shift, value) in source.terms().keys().zip(&evaluated) {
            let Some(q) = raw_q.get(shift.values()) else {
                continue;
            };
            paired += 1;
            residual = sampled
                .field()
                .add(&residual, &sampled.field().mul(value, q));
        }
        if !sampled.field().is_zero(&residual) {
            expected.push(request.clone());
        }
    }

    assert_eq!(retained.requests(), expected);
    assert_eq!(
        (
            nominations.requests().len(),
            retained.requests().len(),
            retained.evaluated_source_terms(),
            retained.paired_source_terms(),
            retained.obstruction_support_entries(),
        ),
        (2, 1, 4, 2, 2),
        "the one-loop complete-row residual census is frozen",
    );
    assert_eq!(
        retained.evaluated_candidates(),
        nominations.requests().len()
    );
    assert_eq!(
        retained.evaluated_source_terms(),
        selected
            .sources()
            .iter()
            .map(|source| source.terms().len())
            .sum::<usize>()
    );
    assert_eq!(retained.paired_source_terms(), paired);
    assert_eq!(
        retained.obstruction_support_entries(),
        obstruction.entries().len()
    );
    assert!(retained.requests().windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        retained.requests().len() < nominations.requests().len(),
        "the fixture must exercise complete-row modular cancellation"
    );
}

#[test]
fn residual_pairing_rejoins_exact_plan_sample_and_generator_scope() {
    let family = one_loop_vacuum("source-discovery-residual-joins");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let first_chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let first_sample = first_chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let first_obstruction = all_other_no_hit(&first_sample);
    let nominations = incidence
        .try_nominate_obstruction(&first_obstruction, limits)
        .unwrap();

    let independent_same_point = first_chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    assert_eq!(independent_same_point.point(), first_sample.point());
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &independent_same_point,
                &first_obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::ObstructionSampleMismatch
    );

    let foreign_field_sample = first_chart
        .plan()
        .try_modular_sample(
            generator.context(),
            1_000_000_009,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    assert_ne!(
        foreign_field_sample.field().get_prime(),
        first_sample.field().get_prime()
    );
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &foreign_field_sample,
                &first_obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::ObstructionSampleMismatch
    );

    let second_chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    assert_eq!(first_chart.plan(), second_chart.plan());
    let second_sample = second_chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let second_obstruction = all_other_no_hit(&second_sample);
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &first_sample,
                &second_obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::ObstructionPlanMismatch
    );

    let foreign_family = one_loop_vacuum("source-discovery-residual-foreign-generator");
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let foreign_completed = complete_ordinary(&foreign_generator);
    assert!(matches!(
        incidence.try_retain_nonzero_residuals(
            &foreign_generator,
            &foreign_completed,
            &nominations,
            &first_sample,
            &first_obstruction,
            limits,
        ),
        Err(SourceDiscoveryError::ScopeMismatch { .. })
    ));
}

#[test]
fn residual_pairing_requires_exact_incidence_origin_and_obstruction_query() {
    let family = one_loop_vacuum("source-discovery-residual-admission-seals");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let equal_shaped_foreign_incidence =
        OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();

    assert_eq!(
        equal_shaped_foreign_incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &sampled,
                &obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::NominationIncidenceMismatch
    );

    let target_unit = incidence
        .try_nominate_target_unit(&IntegralShift::try_new([0]).unwrap(), limits)
        .unwrap();
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &target_unit,
                &sampled,
                &obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::TargetUnitNominationForObstruction
    );

    let different_obstruction =
        different_target_all_other_no_hit(&sampled, obstruction.target_physical_column());
    assert_ne!(
        different_obstruction.target_physical_column(),
        obstruction.target_physical_column()
    );
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &sampled,
                &different_obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::NominationObstructionMismatch
    );
}

#[test]
fn empty_residual_nominations_still_authenticate_scope_and_chronology() {
    let family = one_loop_vacuum("source-discovery-empty-residual-admission");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let foreign_incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let empty = empty_obstruction_nominations_for_test(&incidence, &obstruction).unwrap();
    let retained = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &empty,
            &sampled,
            &obstruction,
            limits,
        )
        .unwrap();
    assert!(retained.requests().is_empty());
    assert_eq!(retained.evaluated_candidates(), 0);
    assert_eq!(
        retained.obstruction_support_entries(),
        obstruction.entries().len()
    );

    let foreign_empty =
        empty_obstruction_nominations_for_test(&foreign_incidence, &obstruction).unwrap();
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &foreign_empty,
                &sampled,
                &obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::NominationIncidenceMismatch
    );

    let foreign_family = one_loop_vacuum("source-discovery-empty-residual-foreign-family");
    let foreign_generator = ParametricIbpGenerator::try_new(&foreign_family).unwrap();
    let foreign_completed = complete_ordinary(&foreign_generator);
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &foreign_completed,
                &empty,
                &sampled,
                &obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::SourceTranslation(
            TranslatedSourceError::CompletedSourceFamilyMismatch
        )
    );

    let chronological_family = one_loop_one_external("source-discovery-empty-residual-chronology");
    let chronological_generator = ParametricIbpGenerator::try_new(&chronological_family).unwrap();
    let mut chronological_completed = complete_ordinary(&chronological_generator);
    let chronological_sources = chronological_generator
        .translate_completed_source_rows(
            &chronological_completed,
            [IntegralShift::try_new([0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let chronological_incidence =
        OrdinarySourceIncidenceIndex::try_new(&chronological_sources, limits).unwrap();
    let chronological_chart = OneSidedChartFrame::try_new(
        &chronological_generator,
        &chronological_completed,
        Mask::try_new([true, false]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let chronological_sample = chronological_chart
        .plan()
        .try_modular_sample(
            chronological_generator.context(),
            PRIME,
            &[37, 5],
            &[0, 0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let chronological_obstruction = all_other_no_hit(&chronological_sample);
    let chronological_empty = empty_obstruction_nominations_for_test(
        &chronological_incidence,
        &chronological_obstruction,
    )
    .unwrap();
    assert_eq!(chronological_completed.source_row_count(), 2);
    assert!(chronological_completed.swap_source_rows_for_test(0, 1));
    assert_eq!(
        chronological_incidence
            .try_retain_nonzero_residuals(
                &chronological_generator,
                &chronological_completed,
                &chronological_empty,
                &chronological_sample,
                &chronological_obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::CompletedSourceChronologyMismatch
    );
}

#[test]
fn residual_pairing_rejects_selected_request_and_row_provenance_mutants() {
    let family = one_loop_vacuum("source-discovery-selected-residual-provenance");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();
    assert!(nominations.requests().len() >= 2);

    let mut wrong_request = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.translation,
        )
        .unwrap();
    assert!(wrong_request.swap_source_provenance_for_test(0, 1));
    assert_eq!(
        pair_selected_sources_for_test(
            &incidence,
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            wrong_request,
            limits,
        )
        .unwrap_err(),
        SourceDiscoveryError::SelectedRequestProvenanceMismatch {
            candidate_ordinal: 0,
        }
    );

    let mut wrong_row = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.translation,
        )
        .unwrap();
    assert!(wrong_row.replace_source_row_id_for_test(
        0,
        RowId::Derived {
            label: Arc::from("foreign-residual-row"),
        },
    ));
    assert_eq!(
        pair_selected_sources_for_test(
            &incidence,
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            wrong_row,
            limits,
        )
        .unwrap_err(),
        SourceDiscoveryError::SelectedSourceRowMismatch {
            candidate_ordinal: 0,
            source_ordinal: nominations.requests()[0].source_ordinal(),
        }
    );
}

#[test]
fn residual_pairing_rejects_external_only_completed_rows_before_translation() {
    let family = one_loop_one_external("source-discovery-residual-layout");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let ordinary = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &ordinary,
            [IntegralShift::try_new([0, 0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &ordinary,
        Mask::try_new([true, false]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37, 5],
            &[0, 0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();
    let external = complete_external(&generator);

    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &external,
                &nominations,
                &sampled,
                &obstruction,
                limits,
            )
            .unwrap_err(),
        SourceDiscoveryError::WrongSourceLayout {
            actual: "external-contraction IBP source",
        }
    );
}

#[test]
fn residual_resource_caps_admit_exact_boundaries_and_fail_transactionally() {
    let family = one_loop_vacuum("source-discovery-residual-limits");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let defaults = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, defaults).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, defaults)
        .unwrap();
    let measured = incidence
        .try_retain_nonzero_residuals(
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            defaults,
        )
        .unwrap();
    assert!(!measured.requests().is_empty());

    let support_coordinates = measured.obstruction_support_entries() * incidence.arity();
    let mut boundary = defaults;
    boundary.max_obstruction_support = measured.obstruction_support_entries();
    boundary.max_residual_candidates = measured.evaluated_candidates();
    boundary.max_residual_source_terms = measured.evaluated_source_terms();
    boundary.max_residual_support_coordinate_cells = support_coordinates;
    boundary.max_residual_classifications = measured.requests().len();
    boundary.max_nonzero_residual_requests = measured.requests().len();
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &sampled,
                &obstruction,
                boundary,
            )
            .unwrap(),
        measured
    );

    for (limits, resource, requested, limit) in [
        (
            {
                let mut value = boundary;
                value.max_residual_candidates -= 1;
                value
            },
            "source-discovery residual candidates",
            measured.evaluated_candidates(),
            measured.evaluated_candidates() - 1,
        ),
        (
            {
                let mut value = boundary;
                value.max_residual_source_terms -= 1;
                value
            },
            "source-discovery residual exact-source terms",
            measured.evaluated_source_terms(),
            measured.evaluated_source_terms() - 1,
        ),
        (
            {
                let mut value = boundary;
                value.max_residual_support_coordinate_cells -= 1;
                value
            },
            "source-discovery residual obstruction-support coordinate cells",
            support_coordinates,
            support_coordinates - 1,
        ),
        (
            {
                let mut value = boundary;
                value.max_residual_classifications -= 1;
                value
            },
            "source-discovery nonzero proposal-score rows",
            measured.requests().len(),
            measured.requests().len() - 1,
        ),
        (
            {
                let mut value = boundary;
                value.max_nonzero_residual_requests -= 1;
                value
            },
            "source-discovery nonzero residual requests",
            measured.requests().len(),
            measured.requests().len() - 1,
        ),
    ] {
        assert_eq!(
            incidence
                .try_retain_nonzero_residuals(
                    &generator,
                    &completed,
                    &nominations,
                    &sampled,
                    &obstruction,
                    limits,
                )
                .unwrap_err(),
            SourceDiscoveryError::ResourceLimit {
                resource,
                requested,
                limit,
            }
        );
    }

    // A failed call retained no mutable campaign state: the same immutable
    // nominations and admitted sample still reproduce the original payload.
    assert_eq!(
        incidence
            .try_retain_nonzero_residuals(
                &generator,
                &completed,
                &nominations,
                &sampled,
                &obstruction,
                boundary,
            )
            .unwrap(),
        measured
    );
}

#[test]
fn off_support_denominator_singularity_is_evaluated_before_sparse_pairing() {
    let family = one_loop_vacuum("source-discovery-residual-off-support-denominator");
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let completed = complete_ordinary(&generator);
    let sources = generator
        .translate_completed_source_rows(
            &completed,
            [IntegralShift::try_new([0]).unwrap()],
            TranslatedSourceLimits::default(),
        )
        .unwrap();
    let limits = SourceDiscoveryLimits::default();
    let incidence = OrdinarySourceIncidenceIndex::try_new(&sources, limits).unwrap();
    let chart = OneSidedChartFrame::try_new(
        &generator,
        &completed,
        Mask::try_new([true]).unwrap(),
        0,
        PhysicalFrameLimits::default(),
    )
    .unwrap();
    let sampled = chart
        .plan()
        .try_modular_sample(
            generator.context(),
            PRIME,
            &[37],
            &[0],
            ModularKernelLimits::default(),
        )
        .unwrap();
    let obstruction = all_other_no_hit(&sampled);
    let nominations = incidence
        .try_nominate_obstruction(&obstruction, limits)
        .unwrap();
    let mut selected = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.translation,
        )
        .unwrap();

    let support = obstruction
        .entries()
        .iter()
        .map(|entry| {
            let physical = obstruction.logical_physical_columns()[entry.logical_column()];
            sampled.plan().columns()[physical].values().to_vec()
        })
        .collect::<std::collections::BTreeSet<_>>();
    let (candidate_ordinal, term_ordinal) = selected
        .sources()
        .iter()
        .enumerate()
        .find_map(|(candidate_ordinal, source)| {
            source
                .terms()
                .keys()
                .enumerate()
                .find(|(_, shift)| !support.contains(shift.values()))
                .map(|(term_ordinal, _)| (candidate_ordinal, term_ordinal))
        })
        .expect("the inverse-incidence fixture must retain a complete-row term outside q support");
    let singular = generator
        .context()
        .lift(&generator.context().base().coefficient_fixture("1/(d-37)"))
        .unwrap();
    selected
        .replace_term_without_denominator_gate_for_test(
            generator.context(),
            candidate_ordinal,
            term_ordinal,
            singular,
        )
        .unwrap();

    assert_eq!(
        pair_selected_sources_for_test(
            &incidence,
            &generator,
            &completed,
            &nominations,
            &sampled,
            &obstruction,
            selected,
            limits,
        )
        .unwrap_err(),
        SourceDiscoveryError::CandidateEvaluation {
            candidate_ordinal,
            source_ordinal: nominations.requests()[candidate_ordinal].source_ordinal(),
            error: ModularSourceEvaluationError::TermDenominatorZero { term_ordinal },
        }
    );

    // The failure cannot leak a retained prefix or mutate the immutable
    // nomination/sample inputs: a clean retranslation still pairs normally.
    let clean = generator
        .translate_selected_completed_source_rows(
            &completed,
            nominations.requests().iter().cloned(),
            limits.translation,
        )
        .unwrap();
    pair_selected_sources_for_test(
        &incidence,
        &generator,
        &completed,
        &nominations,
        &sampled,
        &obstruction,
        clean,
        limits,
    )
    .unwrap();
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
