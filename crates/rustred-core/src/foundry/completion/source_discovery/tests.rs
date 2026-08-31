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
    boundary.max_residual_classifications = measured.evaluated_candidates();
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
            "source-discovery residual candidate classifications",
            measured.evaluated_candidates(),
            measured.evaluated_candidates() - 1,
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
